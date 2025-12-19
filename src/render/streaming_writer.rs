// src/render/streaming_writer.rs
use lopdf::content::Content;
use lopdf::{dictionary, Dictionary, Object, ObjectId, Stream};
use std::collections::BTreeMap;
use std::io::{self, Seek, Write};

pub struct StreamingPdfWriter<W: Write + Seek> {
    writer: W,
    object_offsets: Vec<u64>,
    current_id: u32,

    pub catalog_id: ObjectId,
    pub pages_id: ObjectId,
    pub resources_id: ObjectId,

    page_ids: Vec<ObjectId>,
    outline_root_id: Option<ObjectId>,

    buffered_objects: BTreeMap<ObjectId, Object>,
}

impl<W: Write + Seek> StreamingPdfWriter<W> {
    pub fn new(mut writer: W, version: &str, font_dict: Dictionary) -> io::Result<Self> {
        writer.write_all(format!("%PDF-{}\n%âãÏÓ\n", version).as_bytes())?;

        let resources_id = (1, 0);
        let pages_id = (2, 0);
        let catalog_id = (3, 0);
        let current_id = 3;

        // Initialize offsets with 0 for the reserved IDs
        let object_offsets = vec![0, 0, 0];

        let mut buffered_objects = BTreeMap::new();
        buffered_objects.insert(resources_id, dictionary! { "Font" => font_dict }.into());

        Ok(Self {
            writer,
            object_offsets,
            current_id,
            catalog_id,
            pages_id,
            resources_id,
            page_ids: Vec::new(),
            outline_root_id: None,
            buffered_objects,
        })
    }

    pub fn new_object_id(&mut self) -> ObjectId {
        self.current_id += 1;
        self.object_offsets.push(0);
        (self.current_id, 0)
    }

    pub fn write_object(&mut self, object: Object) -> io::Result<ObjectId> {
        let id = self.new_object_id();
        self.write_object_at_id(id, &object)?;
        Ok(id)
    }

    fn write_object_at_id(&mut self, id: ObjectId, object: &Object) -> io::Result<()> {
        let offset = self.writer.stream_position()?;

        let idx = (id.0 as usize).checked_sub(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Invalid Object ID 0")
        })?;

        if idx >= self.object_offsets.len() {
            self.object_offsets.resize(idx + 1, 0);
        }
        self.object_offsets[idx] = offset;

        internal_writer::write_indirect_object_header(&mut self.writer, id)?;
        internal_writer::write_object(&mut self.writer, object)?;
        internal_writer::write_indirect_object_footer(&mut self.writer)?;

        Ok(())
    }

    pub fn write_content_stream(&mut self, content: Content) -> io::Result<ObjectId> {
        let stream = Stream::new(dictionary! {}, content.encode().unwrap_or_default());
        self.write_object(Object::Stream(stream))
    }

    pub fn buffer_object_at_id(&mut self, id: ObjectId, object: Object) {
        let idx = (id.0 as usize).saturating_sub(1);
        if idx >= self.object_offsets.len() {
            self.object_offsets.resize(idx + 1, 0);
            if id.0 > self.current_id {
                self.current_id = id.0;
            }
        }
        self.buffered_objects.insert(id, object);
    }

    #[allow(dead_code)]
    pub fn buffer_object(&mut self, object: Object) -> ObjectId {
        let id = self.new_object_id();
        self.buffer_object_at_id(id, object);
        id
    }

    pub fn set_page_ids(&mut self, page_ids: Vec<ObjectId>) {
        self.page_ids = page_ids;
    }

    pub fn set_outline_root_id(&mut self, outline_root_id: Option<ObjectId>) {
        self.outline_root_id = outline_root_id;
    }

    pub fn finish(mut self) -> io::Result<W> {
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => self.page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<Object>>(),
            "Count" => self.page_ids.len() as i64,
        };
        self.buffer_object_at_id(self.pages_id, pages_dict.into());

        let mut catalog_dict = dictionary! { "Type" => "Catalog", "Pages" => self.pages_id };
        if let Some(outline_id) = self.outline_root_id {
            catalog_dict.set("Outlines", outline_id);
            catalog_dict.set("PageMode", "UseOutlines");
        }
        self.buffer_object_at_id(self.catalog_id, catalog_dict.into());

        let buffered = std::mem::take(&mut self.buffered_objects);
        for (id, object) in buffered {
            self.write_object_at_id(id, &object)?;
        }

        let xref_start = self.writer.stream_position()?;
        writeln!(self.writer, "xref")?;
        writeln!(self.writer, "0 {}", self.object_offsets.len() + 1)?;
        writeln!(self.writer, "0000000000 65535 f ")?;

        for offset in &self.object_offsets {
            writeln!(self.writer, "{:010} 00000 n ", offset)?;
        }

        let trailer = dictionary! {
            "Size" => (self.object_offsets.len() + 1) as i64,
            "Root" => self.catalog_id
        };
        writeln!(self.writer, "trailer")?;
        internal_writer::write_dictionary(&mut self.writer, &trailer)?;
        writeln!(self.writer, "\nstartxref")?;
        writeln!(self.writer, "{}", xref_start)?;
        write!(self.writer, "%%EOF")?;

        self.writer.flush()?;
        Ok(self.writer)
    }
}

mod internal_writer {
    use super::*;
    use lopdf::StringFormat;
    use std::collections::BTreeMap;

    pub fn write_indirect_object_header<W: Write>(writer: &mut W, id: ObjectId) -> io::Result<()> {
        write!(writer, "{} {} obj\n", id.0, id.1)
    }

    pub fn write_indirect_object_footer<W: Write>(writer: &mut W) -> io::Result<()> {
        writeln!(writer, "\nendobj")
    }

    pub fn write_object(writer: &mut dyn Write, object: &Object) -> io::Result<()> {
        match object {
            Object::Null => writer.write_all(b"null"),
            Object::Boolean(b) => writer.write_all(if *b { b"true" } else { b"false" }),
            Object::Integer(i) => write!(writer, "{}", i),
            Object::Real(r) => write!(writer, "{:.3}", r),
            Object::Name(n) => {
                writer.write_all(b"/")?;
                writer.write_all(n)
            }
            Object::String(s, format) => match format {
                StringFormat::Literal => {
                    writer.write_all(b"(")?;
                    for &byte in s {
                        if byte == b'(' || byte == b')' || byte == b'\\' {
                            writer.write_all(b"\\")?;
                        }
                        writer.write_all(&[byte])?;
                    }
                    writer.write_all(b")")
                }
                StringFormat::Hexadecimal => {
                    write!(writer, "<{}>", s.iter().map(|b| format!("{:02X}", b)).collect::<String>())
                }
            },
            Object::Array(arr) => {
                writer.write_all(b"[")?;
                for (i, obj) in arr.iter().enumerate() {
                    if i > 0 { writer.write_all(b" ")?; }
                    write_object(writer, obj)?;
                }
                writer.write_all(b"]")
            }
            Object::Dictionary(dict) => write_dictionary(writer, dict),
            Object::Stream(stream) => {
                let mut dict = stream.dict.clone();
                dict.set("Length", stream.content.len() as i64);
                write_dictionary(writer, &dict)?;
                writer.write_all(b"\nstream\n")?;
                writer.write_all(&stream.content)?;
                writer.write_all(b"\nendstream")
            }
            Object::Reference(id) => write!(writer, "{} {} R", id.0, id.1),
        }
    }

    pub fn write_dictionary(writer: &mut dyn Write, dict: &Dictionary) -> io::Result<()> {
        writer.write_all(b"<<")?;
        let sorted_keys: BTreeMap<_, _> = dict.iter().collect();
        for (key, value) in sorted_keys {
            writer.write_all(b"/")?;
            writer.write_all(key)?;
            writer.write_all(b" ")?;
            write_object(writer, value)?;
            writer.write_all(b" ")?;
        }
        writer.write_all(b">>")
    }
}