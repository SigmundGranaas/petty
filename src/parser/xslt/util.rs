// src/parser/xslt/util.rs
use crate::parser::{Location, ParseError};
use quick_xml::events::BytesStart;

pub(super) type OwnedAttributes = Vec<(Vec<u8>, Vec<u8>)>;

/// Parses all attributes from a `BytesStart` event into an owned `Vec`.
pub(super) fn get_owned_attributes(e: &BytesStart) -> Result<OwnedAttributes, ParseError> {
    e.attributes()
        .map(|attr_res| {
            let attr = attr_res?;
            Ok((attr.key.into_inner().to_vec(), attr.value.into_owned()))
        })
        .collect()
}

/// Helper function to convert a byte position to a line and column number.
pub(super) fn get_line_col_from_pos(xml_str: &str, pos: usize) -> (usize, usize) {
    let before = &xml_str[..pos.min(xml_str.len())];
    let line = before.lines().count();
    let col = before.lines().last().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

// --- Attribute Getter Utilities ---

pub(super) fn get_attr_owned_optional(
    attributes: &OwnedAttributes,
    name: &[u8],
) -> Result<Option<String>, ParseError> {
    if let Some((_key, value)) = attributes.iter().find(|(k, _v)| k.as_slice() == name) {
        Ok(Some(
            String::from_utf8(value.clone()).map_err(ParseError::Utf8)?,
        ))
    } else {
        Ok(None)
    }
}

pub(super) fn get_attr_owned_required(
    attributes: &OwnedAttributes,
    name: &[u8],
    tag_name: &[u8],
    pos: usize,
    full_xslt_str: &str,
) -> Result<String, ParseError> {
    get_attr_owned_optional(attributes, name)?.ok_or_else(|| {
        let (line, col) = get_line_col_from_pos(full_xslt_str, pos);
        ParseError::TemplateSyntax {
            msg: format!(
                "Missing required attribute '{}' on <{}>",
                String::from_utf8_lossy(name),
                String::from_utf8_lossy(tag_name)
            ),
            location: Location { line, col },
        }
    })
}