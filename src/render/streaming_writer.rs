use lopdf::content::Content;
use lopdf::xref::{Xref, XrefEntry, XrefType};
use lopdf::{dictionary, Dictionary, Object, ObjectId, Stream};
use std::collections::BTreeMap;
use std::io::{self, Seek, Write};

pub struct StreamingPdfWriter<W: Write + Seek> {
    writer: W,
    xref: Xref,
    max_id: u32,
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

        let mut buffered_objects = BTreeMap::new();
        let mut max_id = 0;

        let resources_id = (max_id + 1, 0);
        let pages_id = (max_id + 2, 0);
        let catalog_id = (max_id + 3, 0);
        max_id = 3;

        buffered_objects.insert(resources_id, dictionary! { "Font" => font_dict }.into());

        Ok(Self {
            writer,
            xref: Xref::new(0, XrefType::CrossReferenceTable),
            max_id,
            catalog_id,
            pages_id,
            resources_id,
            page_ids: Vec::new(),
            outline_root_id: None,
            buffered_objects,
        })
    }

    pub fn new_object_id(&mut self) -> ObjectId {
        self.max_id += 1;
        (self.max_id, 0)
    }

    pub fn buffer_object(&mut self, object: Object) -> ObjectId {
        let id = self.new_object_id();
        self.buffered_objects.insert(id, object);
        id
    }

    pub fn buffer_object_at_id(&mut self, id: ObjectId, object: Object) {
        if id.0 > self.max_id {
            self.max_id = id.0;
        }
        self.buffered_objects.insert(id, object);
    }

    pub fn buffer_content_stream(&mut self, content: Content) -> ObjectId {
        let stream = Stream::new(dictionary! {}, content.encode().unwrap_or_default());
        self.buffer_object(Object::Stream(stream))
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

        for (id, object) in &self.buffered_objects {
            internal_writer::write_indirect_object(&mut self.writer, *id, object, &mut self.xref)?;
        }

        let xref_start = self.writer.stream_position()?;
        self.xref.size = self.max_id + 1;
        internal_writer::write_xref(&mut self.writer, &self.xref)?;

        let trailer = dictionary! { "Size" => self.xref.size as i64, "Root" => self.catalog_id };
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

    pub fn write_indirect_object<W: Write + Seek>(writer: &mut W, id: ObjectId, object: &Object, xref: &mut Xref) -> io::Result<()> {
        let offset = writer.stream_position()?;
        xref.insert(id.0, XrefEntry::Normal { offset: offset as u32, generation: id.1 as u16 });
        write!(writer, "{} {} obj\n", id.0, id.1)?;
        write_object(writer, object)?;
        writeln!(writer, "\nendobj")?;
        Ok(())
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

    pub fn write_xref<W: Write>(writer: &mut W, xref: &Xref) -> io::Result<()> {
        writeln!(writer, "xref")?;
        let mut sorted_entries: Vec<_> = xref.entries.iter().collect();
        sorted_entries.sort_by_key(|(k, _)| *k);

        if sorted_entries.is_empty() {
            writeln!(writer, "0 1")?;
            writeln!(writer, "0000000000 65535 f ")?;
            return Ok(());
        }

        let mut start_id = 0;
        let mut entries_in_section = Vec::new();

        // This closure is now defined to not capture `writer`, but to accept it as an argument.
        // This avoids the borrow checker issue where the closure holds a mutable borrow on `writer`
        // while other code tries to use it.
        let mut write_section = |w: &mut W, start_id: u32, entries: &Vec<XrefEntry>| -> io::Result<()> {
            if entries.is_empty() { return Ok(()); }
            writeln!(w, "{} {}", start_id, entries.len())?;
            for entry in entries {
                if let XrefEntry::Normal { offset, generation } = *entry {
                    writeln!(w, "{:010} {:05} n ", offset, generation)?;
                } else {
                    writeln!(w, "0000000000 65535 f ")?;
                }
            }
            Ok(())
        };

        // The pattern `|(&id, _)| id` caused a reference pattern error with modern match ergonomics.
        // A clearer way to get the ID is to access the tuple element directly and dereference it.
        if sorted_entries.get(0).map(|entry| *entry.0) != Some(0) {
            writeln!(writer, "0 1")?;
            writeln!(writer, "0000000000 65535 f ")?;
        }

        // Iterating over `sorted_entries` gives `(&u32, &XrefEntry)` tuples.
        // The pattern `(&id, entry)` destructures this, binding `id` to `u32` and `entry` to `&XrefEntry`.
        for (&id, entry) in sorted_entries {
            if id > 0 && id != start_id + entries_in_section.len() as u32 {
                // Pass the writer explicitly to the closure.
                write_section(writer, start_id, &entries_in_section)?;
                entries_in_section.clear();
            }
            if entries_in_section.is_empty() && id > 0 {
                start_id = id;
            }
            entries_in_section.push(entry.clone());
        }
        // Pass the writer explicitly to the final closure call.
        write_section(writer, start_id, &entries_in_section)?;
        Ok(())
    }
}