use crate::error::PipelineError;
use crate::stylesheet::{Dimension, TableColumn};
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;

pub(super) type OwnedAttributes = Vec<(Vec<u8>, Vec<u8>)>;

/// Parses a string like "50%" or "120pt" into a Dimension enum.
pub(super) fn parse_dimension(s: &str) -> Option<Dimension> {
    if s.ends_with('%') {
        s.trim_end_matches('%')
            .parse::<f32>()
            .ok()
            .map(Dimension::Percent)
    } else if s.ends_with("pt") {
        s.trim_end_matches("pt")
            .parse::<f32>()
            .ok()
            .map(Dimension::Pt)
    } else {
        s.parse::<f32>().ok().map(Dimension::Pt) // Default to Pt
    }
}

/// Parses the children of a `<columns>` tag into a `Vec<TableColumn>`.
pub(super) fn parse_table_columns(
    reader: &mut Reader<&[u8]>,
    end_tag: QName,
) -> Result<Vec<TableColumn>, PipelineError> {
    let mut columns = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            XmlEvent::Empty(e) if e.name().as_ref() == b"column" => {
                let mut col = TableColumn::default();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"width" => {
                            col.width =
                                parse_dimension(&attr.decode_and_unescape_value(reader.decoder())?)
                        }
                        b"header-style" => {
                            col.header_style =
                                Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned())
                        }
                        b"style" => {
                            col.style =
                                Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned())
                        }
                        _ => {}
                    }
                }
                columns.push(col);
            }
            XmlEvent::End(e) if e.name() == end_tag => break,
            XmlEvent::Eof => {
                return Err(PipelineError::TemplateParseError(
                    "Unexpected EOF in columns".into(),
                ))
            }
            _ => (),
        }
        buf.clear();
    }
    Ok(columns)
}

// --- Attribute Getter Utilities ---

pub(super) fn get_attr_owned_optional(
    attributes: &OwnedAttributes,
    name: &[u8],
) -> Result<Option<String>, PipelineError> {
    if let Some((_key, value)) = attributes.iter().find(|(k, _v)| k.as_slice() == name) {
        Ok(Some(
            String::from_utf8(value.clone())
                .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?,
        ))
    } else {
        Ok(None)
    }
}

pub(super) fn get_attr_owned_required(
    attributes: &OwnedAttributes,
    name: &[u8],
    tag_name: &[u8],
) -> Result<String, PipelineError> {
    get_attr_owned_optional(attributes, name)?.ok_or_else(|| {
        PipelineError::TemplateParseError(format!(
            "Missing required attribute '{}' on <{}>",
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(tag_name)
        ))
    })
}

pub(super) fn get_attr_required(e: &BytesStart, name: &[u8]) -> Result<String, PipelineError> {
    let attr = e
        .try_get_attribute(name)?
        .ok_or_else(|| {
            PipelineError::TemplateParseError(format!(
                "Missing required attribute '{}' on <{}>",
                String::from_utf8_lossy(name),
                String::from_utf8_lossy(e.name().as_ref())
            ))
        })?;
    let value = attr
        .unescape_value()
        .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?;
    Ok(value.into_owned())
}