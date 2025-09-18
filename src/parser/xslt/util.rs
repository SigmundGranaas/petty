use crate::error::PipelineError;
use crate::stylesheet::{
    Border, Color, Dimension, ElementStyle, FontStyle, FontWeight, Margins, TableColumn,
    TextAlign,
};
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use std::str::FromStr;

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
                    let value = attr.decode_and_unescape_value(reader.decoder())?;
                    match attr.key.as_ref() {
                        b"width" => col.width = parse_dimension(&value),
                        b"header-style" => col.header_style = Some(value.into_owned()),
                        b"style" => col.style = Some(value.into_owned()),
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

fn parse_pt_value(s: &str) -> Result<f32, PipelineError> {
    Ok(s.trim_end_matches("pt").trim().parse()?)
}

/// Parses CSS-style shorthand for margin/padding.
fn parse_shorthand_value(s: &str) -> Result<Margins, PipelineError> {
    let parts: Vec<f32> = s
        .split_whitespace()
        .map(parse_pt_value)
        .collect::<Result<Vec<f32>, _>>()?;

    match parts.len() {
        1 => Ok(Margins {
            top: parts[0],
            right: parts[0],
            bottom: parts[0],
            left: parts[0],
        }),
        2 => Ok(Margins {
            top: parts[0],
            right: parts[1],
            bottom: parts[0],
            left: parts[1],
        }),
        4 => Ok(Margins {
            top: parts[0],
            right: parts[1],
            bottom: parts[2],
            left: parts[3],
        }),
        _ => Err(PipelineError::TemplateParseError(format!(
            "Invalid shorthand value count: '{}'",
            s
        ))),
    }
}

/// Parses formatting attributes on an XML tag into a universal `ElementStyle` object.
pub(super) fn parse_fo_attributes_to_element_style(
    attributes: &OwnedAttributes,
) -> Result<Option<ElementStyle>, PipelineError> {
    let mut style = ElementStyle::default();
    let mut margin = Margins::default();
    let mut padding = Margins::default();
    let mut has_style = false;
    let mut has_margin = false;
    let mut has_padding = false;

    for (key, value) in attributes {
        let key_str = String::from_utf8_lossy(key);
        let value_str = String::from_utf8_lossy(value);

        // Skip the 'style' attribute which is for named styles.
        if key_str == "style" {
            continue;
        }

        let result: Result<(), PipelineError> = (|| {
            match key_str.as_ref() {
                "font-family" => style.font_family = Some(value_str.to_string()),
                "font-size" => style.font_size = Some(parse_pt_value(&value_str)?),
                "font-weight" => style.font_weight = Some(FontWeight::from_str(&value_str)?),
                "font-style" => style.font_style = Some(FontStyle::from_str(&value_str)?),
                "line-height" => style.line_height = Some(parse_pt_value(&value_str)?),
                "text-align" => style.text_align = Some(TextAlign::from_str(&value_str)?),
                "color" => style.color = Some(Color::from_str(&value_str)?),
                "background-color" => style.background_color = Some(Color::from_str(&value_str)?),
                "border" => style.border = Some(Border::from_str(&value_str)?),
                "border-top" => style.border_top = Some(Border::from_str(&value_str)?),
                "border-bottom" => style.border_bottom = Some(Border::from_str(&value_str)?),
                "margin" => {
                    has_margin = true;
                    margin = parse_shorthand_value(&value_str)?;
                }
                "margin-top" => {
                    has_margin = true;
                    margin.top = parse_pt_value(&value_str)?;
                }
                "margin-right" => {
                    has_margin = true;
                    margin.right = parse_pt_value(&value_str)?;
                }
                "margin-bottom" => {
                    has_margin = true;
                    margin.bottom = parse_pt_value(&value_str)?;
                }
                "margin-left" => {
                    has_margin = true;
                    margin.left = parse_pt_value(&value_str)?;
                }
                "padding" => {
                    has_padding = true;
                    padding = parse_shorthand_value(&value_str)?;
                }
                "padding-top" => {
                    has_padding = true;
                    padding.top = parse_pt_value(&value_str)?;
                }
                "padding-right" => {
                    has_padding = true;
                    padding.right = parse_pt_value(&value_str)?;
                }
                "padding-bottom" => {
                    has_padding = true;
                    padding.bottom = parse_pt_value(&value_str)?;
                }
                "padding-left" => {
                    has_padding = true;
                    padding.left = parse_pt_value(&value_str)?;
                }
                "width" => {
                    if value_str.trim() == "auto" {
                        style.width = Some(Dimension::Auto);
                    } else if value_str.contains('%') {
                        let val_str = value_str.trim_end_matches('%').trim();
                        style.width = Some(Dimension::Percent(val_str.parse()?));
                    } else {
                        style.width = Some(Dimension::Pt(parse_pt_value(&value_str)?));
                    }
                }
                "height" => style.height = Some(Dimension::Pt(parse_pt_value(&value_str)?)),
                _ => return Ok(()), // Not a style attribute, just ignore.
            };
            has_style = true; // Mark that we found at least one style attribute.
            Ok(())
        })();

        result?;
    }

    if has_margin {
        style.margin = Some(margin);
    }
    if has_padding {
        style.padding = Some(padding);
    }

    if has_style {
        Ok(Some(style))
    } else {
        Ok(None)
    }
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