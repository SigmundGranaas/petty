// FILE: src/parser/xslt/util.rs
use crate::parser::{Location, ParseError};
use crate::parser::xpath;
use crate::parser::xslt::ast::{AttributeValueTemplate, AvtPart};
use quick_xml::events::BytesStart;

pub(super) type OwnedAttributes = Vec<(Vec<u8>, Vec<u8>)>;

/// Parses all attributes from a `BytesStart` event into an owned `Vec`.
pub(crate) fn get_owned_attributes(e: &BytesStart) -> Result<OwnedAttributes, ParseError> {
    e.attributes()
        .map(|attr_res| {
            let attr = attr_res?;
            Ok((attr.key.into_inner().to_vec(), attr.value.into_owned()))
        })
        .collect()
}

/// Helper function to convert a byte position to a line and column number.
pub(crate) fn get_line_col_from_pos(xml_str: &str, pos: usize) -> (usize, usize) {
    let before = &xml_str[..pos.min(xml_str.len())];
    let line = before.lines().count();
    let col = before.lines().last().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

/// Parses an Attribute Value Template string like "Hello {user/name}" into parts.
pub(crate) fn parse_avt(text: &str) -> Result<AttributeValueTemplate, ParseError> {
    if !text.contains('{') {
        return Ok(AttributeValueTemplate::Static(text.to_string()));
    }

    let mut parts = Vec::new();
    let mut last_end = 0;
    for (start, _part) in text.match_indices('{') {
        // Disregard escaped curly braces `{{`
        if text.get(start + 1..start + 2) == Some("{") {
            continue;
        }
        if start > last_end {
            parts.push(AvtPart::Static(text[last_end..start].replace("{{", "{").replace("}}", "}")));
        }
        let end_marker = "}";
        let end = text[start..]
            .find(end_marker)
            .ok_or_else(|| ParseError::TemplateParse("Unclosed { expression in AVT".to_string()))?;
        let inner = text[start + 1..start + end].trim();

        let expression = xpath::parse_expression(inner)?;
        parts.push(AvtPart::Dynamic(expression));
        last_end = start + end + 1;
    }
    if last_end < text.len() {
        parts.push(AvtPart::Static(text[last_end..].replace("{{", "{").replace("}}", "}")));
    }

    Ok(AttributeValueTemplate::Dynamic(parts))
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