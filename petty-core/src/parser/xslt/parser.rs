// FILE: src/parser/xslt/parser.rs
//! A "dumb" XML driver that reads an XSLT source file and notifies a builder object of events.
use super::compiler::StylesheetBuilder;
use super::util::get_owned_attributes;
use crate::parser::ParseError;
use quick_xml::events::{Event as XmlEvent};
use quick_xml::Reader;

/// Drives the parsing process, calling builder methods for each significant XML event.
pub fn parse_stylesheet_content(
    source: &str,
    builder: &mut impl StylesheetBuilder,
) -> Result<(), ParseError> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    loop {
        let pos = reader.buffer_position();
        match reader.read_event_into(&mut buf)? {
            XmlEvent::Start(e) => {
                let owned_e = e.to_owned();
                let attributes = get_owned_attributes(&owned_e)?;
                builder.start_element(&owned_e, attributes, pos as usize, source)?;
            }
            XmlEvent::Empty(e) => {
                let owned_e = e.to_owned();
                let attributes = get_owned_attributes(&owned_e)?;
                builder.empty_element(&owned_e, attributes, pos as usize, source)?;
            }
            XmlEvent::Text(e) => {
                let text = e.unescape()?.into_owned();
                builder.text(text)?;
            }
            XmlEvent::End(e) => {
                builder.end_element(&e.to_owned(), pos as usize, source)?;
            }
            XmlEvent::Eof => break,
            _ => (),
        }
        buf.clear();
    }

    Ok(())
}