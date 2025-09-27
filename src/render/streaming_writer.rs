// FILE: /home/sigmund/RustroverProjects/petty/src/render/streaming_writer.rs
// src/render/streaming_writer.rs

use lopdf::content::Content;
use lopdf::xref::{Xref, XrefEntry, XrefType};
use lopdf::{dictionary, Dictionary, Object, ObjectId, Stream};
use std::io::{self, Write};

/// Manages the process of writing a PDF document to a stream incrementally.
pub struct StreamingPdfWriter<W: Write> {
    writer: internal_writer::CountingWrite<W>,
    xref: Xref,
    max_id: u32,
    pub page_ids: Vec<ObjectId>,
    catalog_id: ObjectId,
    pub pages_id: ObjectId,
    pub resources_id: ObjectId,
}

impl<W: Write> StreamingPdfWriter<W> {
    /// Creates a new streaming writer and immediately writes the PDF header and
    /// essential document scaffolding (Catalog, Pages, Resources) to the output stream.
    pub fn new(writer: W, version: &str, font_dict: Dictionary) -> io::Result<Self> { // <-- MODIFIED
        let mut writer = internal_writer::CountingWrite::new(writer);

        writeln!(writer, "%PDF-{}", version)?;
        writeln!(writer, "%âãÏÓ")?; // Binary mark for PDF/A compatibility

        let mut xref = Xref::new(0, XrefType::CrossReferenceTable);

        // Reserve Object IDs
        let catalog_id = (1, 0);
        let pages_id = (2, 0);
        let resources_id = (3, 0);
        let max_id = 3;

        // --- Resources Object ---
        let resources_dict = dictionary! {
            "Font" => font_dict, // <-- USE THE PASSED-IN DICTIONARY
        };
        internal_writer::write_indirect_object(&mut writer, resources_id.0, resources_id.1, &resources_dict.into(), &mut xref)?;

        // --- Initial (empty) Pages Object ---
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => 0,
        };
        internal_writer::write_indirect_object(&mut writer, pages_id.0, pages_id.1, &pages_dict.into(), &mut xref)?;

        // --- Catalog Object ---
        let catalog_dict = dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        };
        internal_writer::write_indirect_object(&mut writer, catalog_id.0, catalog_id.1, &catalog_dict.into(), &mut xref)?;

        Ok(Self {
            writer,
            xref,
            max_id,
            page_ids: Vec::new(),
            catalog_id,
            pages_id,
            resources_id,
        })
    }

    /// Generates a new unique object ID for the document.
    pub fn new_object_id(&mut self) -> ObjectId {
        self.max_id += 1;
        (self.max_id, 0)
    }

    pub fn add_page_ids(&mut self, ids: impl Iterator<Item = ObjectId>) {
        self.page_ids.extend(ids);
    }

    pub fn write_pre_rendered_objects(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        self.writer.write_all(&bytes)
    }

    /// Renders a single page and writes its objects directly to the output stream.
    pub fn add_page(&mut self, content: Content, media_box: [f32; 4]) -> io::Result<()> {
        // --- Page Content Stream ---
        let content_stream = Stream::new(dictionary!{}, content.encode().unwrap());
        let content_id = self.new_object_id();
        internal_writer::write_indirect_object(&mut self.writer, content_id.0, content_id.1, &content_stream.into(), &mut self.xref)?;

        // --- Page Object ---
        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => self.pages_id,
            "MediaBox" => media_box.iter().map(|&v| v.into()).collect::<Vec<Object>>(),
            "Contents" => content_id,
            "Resources" => self.resources_id,
        };
        let page_id = self.new_object_id();
        internal_writer::write_indirect_object(&mut self.writer, page_id.0, page_id.1, &page_dict.into(), &mut self.xref)?;

        self.page_ids.push(page_id);
        Ok(())
    }

    /// Finalizes the document by writing the updated Pages object, XRef table, and trailer.
    pub fn finish(mut self) -> io::Result<()> {
        // --- Update the Pages object ---
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => self.page_ids.iter().map(|&id| Object::from(id)).collect::<Vec<Object>>(),
            "Count" => self.page_ids.len() as i32,
        };
        internal_writer::write_indirect_object(&mut self.writer, self.pages_id.0, self.pages_id.1, &pages_dict.into(), &mut self.xref)?;

        // --- Write XRef Table and Trailer ---
        let xref_start = self.writer.bytes_written;
        self.xref.size = self.max_id + 1;

        internal_writer::write_xref(&mut self.writer, &self.xref)?;

        let trailer = dictionary! {
            "Size" => self.xref.size as i64,
            "Root" => self.catalog_id,
        };

        writeln!(self.writer, "trailer")?;
        internal_writer::write_dictionary(&mut self.writer, &trailer)?;
        writeln!(self.writer, "\nstartxref")?;
        writeln!(self.writer, "{}", xref_start)?;
        write!(self.writer, "%%EOF")?;

        self.writer.flush()
    }
}

/// This internal module replicates the necessary functions from `lopdf::writer`
/// to avoid needing to fork the library.
pub(crate) mod internal_writer {
    use super::*;
    use lopdf::StringFormat;

    pub struct CountingWrite<W: Write> {
        pub inner: W,
        pub bytes_written: usize,
    }

    impl<W: Write> CountingWrite<W> {
        pub fn new(inner: W) -> Self {
            Self { inner, bytes_written: 0 }
        }
    }

    impl<W: Write> Write for CountingWrite<W> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let bytes = self.inner.write(buf)?;
            self.bytes_written += bytes;
            Ok(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }

    pub fn write_indirect_object<W: Write>(
        file: &mut CountingWrite<W>, id: u32, generation: u16, object: &Object, xref: &mut Xref,
    ) -> io::Result<()> {
        let offset = file.bytes_written as u32;
        xref.insert(id, XrefEntry::Normal { offset, generation });
        write!(file, "{} {} obj\n", id, generation)?;
        write_object(file, object)?;
        writeln!(file, "\nendobj")?;
        Ok(())
    }

    pub fn write_object(file: &mut dyn Write, object: &Object) -> io::Result<()> {
        match object {
            Object::Null => file.write_all(b"null"),
            Object::Boolean(value) => file.write_all(if *value { b"true" } else { b"false" }),
            Object::Integer(value) => {
                let mut buf = itoa::Buffer::new();
                file.write_all(buf.format(*value).as_bytes())
            }
            Object::Real(value) => write!(file, "{value}"),
            Object::Name(name) => write_name(file, name),
            Object::String(text, format) => write_string(file, text, format),
            Object::Array(array) => write_array(file, array),
            Object::Dictionary(dict) => write_dictionary(file, dict),
            Object::Stream(stream) => write_stream(file, stream),
            Object::Reference(id) => write!(file, "{} {} R", id.0, id.1),
        }
    }

    fn write_name(file: &mut dyn Write, name: &[u8]) -> io::Result<()> {
        file.write_all(b"/")?;
        for &byte in name {
            if b" \t\n\r\x0C()<>[]{}/%#".contains(&byte) || !(33..=126).contains(&byte) {
                write!(file, "#{byte:02X}")?;
            } else {
                file.write_all(&[byte])?;
            }
        }
        Ok(())
    }

    fn write_string(file: &mut dyn Write, text: &[u8], format: &StringFormat) -> io::Result<()> {
        match *format {
            StringFormat::Literal => {
                file.write_all(b"(")?;
                for &byte in text {
                    match byte {
                        b'(' | b')' | b'\\' => {
                            file.write_all(b"\\")?;
                            file.write_all(&[byte])?;
                        }
                        _ => {
                            file.write_all(&[byte])?;
                        }
                    }
                }
                file.write_all(b")")
            }
            StringFormat::Hexadecimal => {
                file.write_all(b"<")?;
                for &byte in text {
                    write!(file, "{byte:02X}")?;
                }
                file.write_all(b">")
            }
        }
    }

    fn write_array(file: &mut dyn Write, array: &[Object]) -> io::Result<()> {
        file.write_all(b"[")?;
        for (i, object) in array.iter().enumerate() {
            if i > 0 {
                file.write_all(b" ")?;
            }
            write_object(file, object)?;
        }
        file.write_all(b"]")
    }

    pub fn write_dictionary(file: &mut dyn Write, dictionary: &Dictionary) -> io::Result<()> {
        file.write_all(b"<<")?;
        for (key, value) in dictionary {
            write_name(file, key)?;
            file.write_all(b" ")?;
            write_object(file, value)?;
        }
        file.write_all(b">>")
    }

    fn write_stream(file: &mut dyn Write, stream: &Stream) -> io::Result<()> {
        write_dictionary(file, &stream.dict)?;
        file.write_all(b"\nstream\n")?;
        file.write_all(&stream.content)?;
        file.write_all(b"\nendstream")
    }

    // --- XrefSection implementation copied from lopdf internals ---
    #[derive(Debug, Clone)]
    struct XrefSection {
        pub start_id: u32,
        pub entries: Vec<XrefEntry>,
    }

    impl XrefSection {
        pub fn new(start_id: u32) -> XrefSection {
            XrefSection {
                start_id,
                entries: vec![],
            }
        }

        pub fn add_entry(&mut self, entry: XrefEntry) {
            self.entries.push(entry);
        }

        pub fn add_unusable_free_entry(&mut self) {
            self.add_entry(XrefEntry::Free);
        }

        pub fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }

        pub fn write_xref_section(&self, file: &mut dyn Write) -> io::Result<()> {
            writeln!(file, "{} {}", self.start_id, self.entries.len())?;
            for entry in &self.entries {
                match *entry {
                    XrefEntry::Normal {
                        offset,
                        generation,
                    } => {
                        writeln!(file, "{:010} {:05} n ", offset, generation)?;
                    }
                    XrefEntry::Free => {
                        writeln!(file, "{:010} {:05} f ", 0, 65535)?;
                    }
                    _ => {}
                }
            }
            Ok(())
        }
    }

    pub fn write_xref(file: &mut dyn Write, xref: &Xref) -> io::Result<()> {
        writeln!(file, "xref")?;
        let mut section = XrefSection::new(0);
        section.add_unusable_free_entry();

        for obj_id in 1..xref.size {
            if section.is_empty() {
                section = XrefSection::new(obj_id);
            }
            if let Some(entry) = xref.get(obj_id) {
                section.add_entry(entry.clone());
            } else {
                if !section.is_empty() {
                    section.write_xref_section(file)?;
                }
                section = XrefSection::new(obj_id);
            }
        }
        if !section.is_empty() {
            section.write_xref_section(file)?;
        }
        Ok(())
    }
}