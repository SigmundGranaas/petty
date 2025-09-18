// src/stylesheet.rs
use crate::error::PipelineError;
use quick_xml::events::attributes::Attributes;
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

// Main struct representing all style and layout information.
// Can be constructed either from a JSON file or by pre-parsing an XSLT file.
#[derive(Debug, Clone, Serialize, Deserialize)] // Removed Default derive
pub struct Stylesheet {
    pub page: PageLayout,
    pub styles: HashMap<String, ElementStyle>,
    // The following fields are only used by the JSON engine
    #[serde(default)]
    pub templates: HashMap<String, Template>,
    #[serde(default)]
    pub page_sequences: HashMap<String, PageSequence>,
}

impl Default for Stylesheet {
    fn default() -> Self {
        Stylesheet {
            page: PageLayout::default(),
            styles: HashMap::new(),
            templates: HashMap::new(),
            page_sequences: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)] // Removed Default derive
pub struct PageLayout {
    #[serde(default)]
    pub title: Option<String>,
    pub size: PageSize,
    pub margins: Margins,
    #[serde(default)]
    pub footer_height: f32,
    pub footer_text: Option<String>,
    pub footer_style: Option<String>,
}

// Modify PageLayout::default() to set default page margins
impl Default for PageLayout {
    fn default() -> Self {
        PageLayout {
            title: None,
            size: PageSize::A4,
            margins: Margins { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 }, // Changed from Default::default()
            footer_height: 0.0,
            footer_text: None,
            footer_style: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)] // Removed Default derive
pub struct Margins {
    #[serde(default)]
    pub top: f32,
    #[serde(default)]
    pub right: f32,
    #[serde(default)]
    pub bottom: f32,
    #[serde(default)]
    pub left: f32,
}

// Keep Margins::default() as 0.0, as it's a generic struct
impl Default for Margins {
    fn default() -> Self {
        Margins {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSequence {
    pub template: String,
    pub data_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PageSize {
    A4,
    Letter,
    Legal,
    Custom { width: f32, height: f32 },
}

impl Default for PageSize {
    fn default() -> Self {
        PageSize::A4
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
// Renamed from 'Style' to 'ElementStyle' to avoid conflict with `std::fmt::Style` or similar
// and to be more descriptive.
pub struct ElementStyle {
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub line_height: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub color: Option<Color>,
    pub margin: Option<Margins>,
    pub padding: Option<Margins>,
    pub width: Option<Dimension>,
    pub height: Option<Dimension>,
    pub background_color: Option<Color>,
    pub border: Option<Border>,
    #[serde(default)]
    pub border_top: Option<Border>,
    #[serde(default)]
    pub border_bottom: Option<Border>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub children: Vec<TemplateElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum TemplateElement {
    Text {
        content: String,
        style: Option<String>,
    },
    Table {
        data_source: String,
        columns: Vec<TableColumn>,
        style: Option<String>,
        row_style_prefix_field: Option<String>,
    },
    Container {
        children: Vec<TemplateElement>,
        style: Option<String>,
        data_source: Option<String>,
    },
    Rectangle {
        style: Option<String>,
    },
    Image {
        src: String,
        style: Option<String>,
    },
    PageBreak,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TableColumn {
    pub header: String,
    pub data_field: String,
    pub width: Option<Dimension>,
    pub style: Option<String>,
    pub header_style: Option<String>,
    pub content_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Dimension {
    Px(f32),
    Pt(f32),
    Percent(f32),
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontWeight {
    Thin,
    Light,
    Regular,
    Medium,
    Bold,
    Black,
    #[serde(untagged)]
    Numeric(u16),
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::Regular
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle::Normal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
}

impl Default for TextAlign {
    fn default() -> Self {
        TextAlign::Left
    }
}

fn default_alpha() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Border {
    pub width: f32,
    pub style: BorderStyle,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BorderStyle {
    Solid,
    Dashed,
    Dotted,
    Double,
    None,
}

impl Stylesheet {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Pre-parses an XSLT file to extract `<fo:simple-page-master>` and `<xsl:attribute-set>` blocks.
    pub fn from_xslt(xslt_content: &str) -> Result<Self, PipelineError> {
        let mut reader = Reader::from_str(xslt_content);
        reader.config_mut().trim_text(true);

        let mut page_layout: Option<PageLayout> = None;
        let mut styles = HashMap::new();
        let mut buf = Vec::new();
        let mut style_buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(e)) | Ok(XmlEvent::Empty(e)) => {
                    match e.name().as_ref() {
                        b"fo:simple-page-master" => {
                            page_layout = Some(parse_simple_page_master(e.attributes())?);
                        }
                        b"xsl:attribute-set" => {
                            let (name, style) =
                                parse_attribute_set(&mut reader, e.attributes(), &mut style_buf)?;
                            styles.insert(name, style);
                        }
                        _ => {}
                    }
                }
                Ok(XmlEvent::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => (),
            }
            buf.clear();
        }

        Ok(Stylesheet {
            page: page_layout.ok_or_else(|| {
                PipelineError::StylesheetError(
                    "Missing <fo:simple-page-master> tag in XSLT.".to_string(),
                )
            })?,
            styles,
            ..Default::default()
        })
    }
}

// --- XSLT Pre-parsing Helper Functions ---

fn get_attr_val(
    attrs: Attributes,
    name: &[u8],
) -> Result<Option<String>, quick_xml::Error> {
    for attr in attrs.flatten() {
        if attr.key.as_ref() == name {
            return Ok(Some(attr.unescape_value()?.into_owned()));
        }
    }
    Ok(None)
}

fn parse_dimension_to_pt(s: &str) -> Result<f32, PipelineError> {
    if let Some(val_str) = s.strip_suffix("pt") {
        Ok(val_str.trim().parse::<f32>()?)
    } else if let Some(val_str) = s.strip_suffix("in") {
        Ok(val_str.trim().parse::<f32>()? * 72.0)
    } else if let Some(val_str) = s.strip_suffix("cm") {
        Ok(val_str.trim().parse::<f32>()? * 28.35)
    } else {
        // Assume points if no unit is specified
        Ok(s.trim().parse::<f32>()?)
    }
}

fn parse_simple_page_master(attrs: Attributes) -> Result<PageLayout, PipelineError> {
    let mut layout = PageLayout::default();
    let mut width: Option<f32> = None;
    let mut height: Option<f32> = None;

    for attr_res in attrs {
        let attr = attr_res?;
        let value = attr.unescape_value()?;
        match attr.key.as_ref() {
            b"page-width" => width = Some(parse_dimension_to_pt(&value)?),
            b"page-height" => height = Some(parse_dimension_to_pt(&value)?),
            b"margin-top" => layout.margins.top = parse_dimension_to_pt(&value)?,
            b"margin-right" => layout.margins.right = parse_dimension_to_pt(&value)?,
            b"margin-bottom" => layout.margins.bottom = parse_dimension_to_pt(&value)?,
            b"margin-left" => layout.margins.left = parse_dimension_to_pt(&value)?,
            b"margin" => layout.margins = parse_shorthand_value(&value)?,
            // These are not standard FO, but useful to keep for footers
            b"footer-text" => layout.footer_text = Some(value.into_owned()),
            b"footer-style" => layout.footer_style = Some(value.into_owned()),
            _ => {}
        }
    }

    if let (Some(w), Some(h)) = (width, height) {
        layout.size = PageSize::Custom { width: w, height: h };
    } else {
        layout.size = PageSize::A4; // Default if not specified
    }

    Ok(layout)
}

/// Helper to apply a parsed attribute value to the style structs.
fn apply_attribute_value(
    style: &mut ElementStyle,
    margin: &mut Margins,
    padding: &mut Margins,
    has_margin: &mut bool,
    has_padding: &mut bool,
    attr_name: &str,
    value: &str,
) -> Result<(), PipelineError> {
    let result: Result<(), PipelineError> = (|| {
        match attr_name {
            "font-family" => {
                style.font_family = Some(value.to_string());
                Ok(())
            }
            "font-size" => {
                style.font_size = Some(parse_pt_value(value)?);
                Ok(())
            }
            "font-weight" => {
                style.font_weight = Some(FontWeight::from_str(value)?);
                Ok(())
            }
            "font-style" => {
                style.font_style = Some(FontStyle::from_str(value)?);
                Ok(())
            }
            "line-height" => {
                style.line_height = Some(parse_pt_value(value)?);
                Ok(())
            }
            "text-align" => {
                style.text_align = Some(TextAlign::from_str(value)?);
                Ok(())
            }
            "color" => {
                style.color = Some(Color::from_str(value)?);
                Ok(())
            }
            "background-color" => {
                style.background_color = Some(Color::from_str(value)?);
                Ok(())
            }
            "border" => {
                style.border = Some(Border::from_str(value)?);
                Ok(())
            }
            "border-top" => {
                style.border_top = Some(Border::from_str(value)?);
                Ok(())
            }
            "border-bottom" => {
                style.border_bottom = Some(Border::from_str(value)?);
                Ok(())
            }
            "margin" => {
                *has_margin = true;
                *margin = parse_shorthand_value(value)?;
                Ok(())
            }
            "margin-top" => {
                *has_margin = true;
                margin.top = parse_pt_value(value)?;
                Ok(())
            }
            "margin-right" => {
                *has_margin = true;
                margin.right = parse_pt_value(value)?;
                Ok(())
            }
            "margin-bottom" => {
                *has_margin = true;
                margin.bottom = parse_pt_value(value)?;
                Ok(())
            }
            "margin-left" => {
                *has_margin = true;
                margin.left = parse_pt_value(value)?;
                Ok(())
            }
            "padding" => {
                *has_padding = true;
                *padding = parse_shorthand_value(value)?;
                Ok(())
            }
            "padding-top" => {
                *has_padding = true;
                padding.top = parse_pt_value(value)?;
                Ok(())
            }
            "padding-right" => {
                *has_padding = true;
                padding.right = parse_pt_value(value)?;
                Ok(())
            }
            "padding-bottom" => {
                *has_padding = true;
                padding.bottom = parse_pt_value(value)?;
                Ok(())
            }
            "padding-left" => {
                *has_padding = true;
                padding.left = parse_pt_value(value)?;
                Ok(())
            }
            "width" => {
                if value.trim() == "auto" {
                    style.width = Some(Dimension::Auto);
                } else if value.contains('%') {
                    let val_str = value.trim_end_matches('%').trim();
                    style.width = Some(Dimension::Percent(val_str.parse()?));
                } else {
                    style.width = Some(Dimension::Pt(parse_pt_value(value)?));
                }
                Ok(())
            }
            "height" => {
                style.height = Some(Dimension::Pt(parse_pt_value(value)?));
                Ok(())
            }
            _ => Ok(()),
        }
    })();

    if let Err(e) = result {
        // If parsing fails, it's likely a template string. Log and ignore the attribute.
        if let PipelineError::FloatParseError(_) = e {
            log::warn!("Could not pre-parse value '{}' for attribute '{}' in XSLT attribute-set. Dynamic values are not supported and will be ignored.", value, attr_name);
        } else {
            // It's a different kind of error, so propagate it.
            return Err(e);
        }
    }
    Ok(())
}

fn parse_attribute_set(
    reader: &mut Reader<&[u8]>,
    attrs: Attributes,
    buf: &mut Vec<u8>,
) -> Result<(String, ElementStyle), PipelineError> {
    let name = get_attr_val(attrs, b"name")?
        .ok_or_else(|| PipelineError::TemplateParseError("xsl:attribute-set missing name".into()))?;
    let mut style = ElementStyle::default();
    let mut margin = Margins::default();
    let mut padding = Margins::default();
    let mut has_margin = false;
    let mut has_padding = false;

    loop {
        match reader.read_event_into(buf) {
            Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"xsl:attribute" => {
                let attr_name = get_attr_val(e.attributes(), b"name")?.ok_or_else(|| {
                    PipelineError::TemplateParseError("xsl:attribute missing name".into())
                })?;

                let mut value = String::new();
                let mut content_buf = Vec::<u8>::new();
                let end_tag = e.name().to_owned();

                // Read content until we hit the end tag for this attribute.
                loop {
                    match reader.read_event_into(&mut content_buf) {
                        Ok(XmlEvent::Text(text)) => {
                            value = text.unescape()?.to_string();
                        }
                        Ok(XmlEvent::End(end)) if end.name() == end_tag => {
                            break;
                        }
                        Ok(XmlEvent::Eof) => return Err(PipelineError::TemplateParseError("Unexpected EOF in xsl:attribute".into())),
                        Err(e) => return Err(e.into()),
                        _ => {} // Ignore comments, etc.
                    }
                    content_buf.clear();
                }
                if !value.trim().is_empty() {
                    apply_attribute_value(&mut style, &mut margin, &mut padding, &mut has_margin, &mut has_padding, &attr_name, &value)?;
                }
            }
            Ok(XmlEvent::Empty(e)) if e.name().as_ref() == b"xsl:attribute" => {
                // An empty attribute tag, like <xsl:attribute name="..."/>. It has no value. We can ignore.
            }
            Ok(XmlEvent::End(e)) if e.name().as_ref() == b"xsl:attribute-set" => break,
            Ok(XmlEvent::Eof) => {
                return Err(PipelineError::TemplateParseError(
                    "Unexpected EOF in xsl:attribute-set".into(),
                ))
            }
            Err(e) => return Err(e.into()),
            _ => (),
        }
        buf.clear();
    }
    if has_margin {
        style.margin = Some(margin);
    }
    if has_padding {
        style.padding = Some(padding);
    }
    Ok((name, style))
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

// --- FromStr implementations for enums and structs ---

impl FromStr for PageSize {
    type Err = PipelineError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "a4" => PageSize::A4,
            "letter" => PageSize::Letter,
            "legal" => PageSize::Legal,
            _ => {
                return Err(PipelineError::TemplateParseError(format!(
                    "Unknown page size: {}",
                    s
                )))
            }
        })
    }
}

impl FromStr for FontWeight {
    type Err = PipelineError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "thin" => FontWeight::Thin,
            "light" => FontWeight::Light,
            "regular" | "normal" => FontWeight::Regular,
            "medium" => FontWeight::Medium,
            "bold" => FontWeight::Bold,
            "black" => FontWeight::Black,
            _ => FontWeight::Numeric(s.parse().map_err(|_| {
                PipelineError::TemplateParseError(format!("Invalid font weight: '{}'", s))
            })?),
        })
    }
}

impl FromStr for FontStyle {
    type Err = PipelineError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "normal" => FontStyle::Normal,
            "italic" => FontStyle::Italic,
            "oblique" => FontStyle::Oblique,
            _ => {
                return Err(PipelineError::TemplateParseError(format!(
                    "Invalid font style: {}",
                    s
                )))
            }
        })
    }
}

impl FromStr for TextAlign {
    type Err = PipelineError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "left" => TextAlign::Left,
            "right" => TextAlign::Right,
            "center" => TextAlign::Center,
            "justify" => TextAlign::Justify,
            _ => {
                return Err(PipelineError::TemplateParseError(format!(
                    "Invalid text align: {}",
                    s
                )))
            }
        })
    }
}

impl FromStr for Color {
    type Err = PipelineError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('#') {
            let hex = s.trim_start_matches('#');
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                return Ok(Color { r, g, b, a: 1.0 });
            } else if hex.len() == 3 {
                let r_char = hex.chars().next().unwrap();
                let g_char = hex.chars().nth(1).unwrap();
                let b_char = hex.chars().nth(2).unwrap();
                let r = u8::from_str_radix(&format!("{}{}", r_char, r_char), 16).unwrap_or(0);
                let g = u8::from_str_radix(&format!("{}{}", g_char, g_char), 16).unwrap_or(0);
                let b = u8::from_str_radix(&format!("{}{}", b_char, b_char), 16).unwrap_or(0);
                return Ok(Color { r, g, b, a: 1.0 });
            }
        }
        Err(PipelineError::TemplateParseError(format!(
            "Invalid color format: '{}'. Use #RRGGBB or #RGB.",
            s
        )))
    }
}

impl FromStr for Border {
    type Err = PipelineError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(PipelineError::TemplateParseError(format!(
                "Invalid border format: '{}'. Use 'width style #RRGGBB'.",
                s
            )));
        }
        Ok(Border {
            width: parse_pt_value(parts[0])?,
            style: BorderStyle::from_str(parts[1])?,
            color: Color::from_str(parts[2])?,
        })
    }
}

impl FromStr for BorderStyle {
    type Err = PipelineError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "solid" => BorderStyle::Solid,
            "dashed" => BorderStyle::Dashed,
            "dotted" => BorderStyle::Dotted,
            "double" => BorderStyle::Double,
            "none" => BorderStyle::None,
            _ => {
                return Err(PipelineError::TemplateParseError(format!(
                    "Invalid border style: {}",
                    s
                )))
            }
        })
    }
}