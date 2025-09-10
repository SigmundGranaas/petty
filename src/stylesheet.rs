use crate::error::PipelineError;
use quick_xml::events::attributes::Attributes;
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

// Main struct representing all style and layout information.
// Can be constructed either from a JSON file or by pre-parsing an XSLT file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stylesheet {
    pub page: PageLayout,
    pub styles: HashMap<String, ElementStyle>,
    // The following fields are only used by the JSON engine
    #[serde(default)]
    pub templates: HashMap<String, Template>,
    #[serde(default)]
    pub page_sequences: HashMap<String, PageSequence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSequence {
    pub template: String,
    pub data_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
#[serde(untagged)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Pre-parses an XSLT file to extract `<petty:page-layout>` and `<xsl:attribute-set>` blocks.
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
                        b"petty:page-layout" => {
                            page_layout = Some(parse_page_layout(e.attributes())?);
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
                    "Missing <petty:page-layout> tag in XSLT.".to_string(),
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

fn parse_page_layout(attrs: Attributes) -> Result<PageLayout, PipelineError> {
    let mut layout = PageLayout::default();
    for attr_res in attrs {
        let attr = attr_res?;
        let value = attr.unescape_value()?;
        match attr.key.as_ref() {
            b"size" => layout.size = PageSize::from_str(&value)?,
            b"margin" => layout.margins = parse_shorthand_value(&value)?,
            b"margin-top" => layout.margins.top = parse_pt_value(&value)?,
            b"margin-right" => layout.margins.right = parse_pt_value(&value)?,
            b"margin-bottom" => layout.margins.bottom = parse_pt_value(&value)?,
            b"margin-left" => layout.margins.left = parse_pt_value(&value)?,
            b"footer-text" => layout.footer_text = Some(value.into_owned()),
            b"footer-style" => layout.footer_style = Some(value.into_owned()),
            _ => {}
        }
    }
    Ok(layout)
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
            Ok(XmlEvent::Start(e)) | Ok(XmlEvent::Empty(e))
            if e.name().as_ref() == b"xsl:attribute" =>
                {
                    let attr_name = get_attr_val(e.attributes(), b"name")?.ok_or_else(|| {
                        PipelineError::TemplateParseError("xsl:attribute missing name".into())
                    })?;
                    // Reading the text content of the xsl:attribute
                    let mut content_buf = Vec::new();
                    let value =
                        if let Ok(XmlEvent::Text(text)) = reader.read_event_into(&mut content_buf) {
                            text.unescape()?.to_string()
                        } else {
                            String::new() // Handle self-closing tags
                        };

                    match attr_name.as_str() {
                        "font-size" => style.font_size = Some(parse_pt_value(&value)?),
                        "font-weight" => style.font_weight = Some(FontWeight::from_str(&value)?),
                        "text-align" => style.text_align = Some(TextAlign::from_str(&value)?),
                        "color" => style.color = Some(Color::from_str(&value)?),
                        "background-color" => {
                            style.background_color = Some(Color::from_str(&value)?)
                        }
                        "border" => style.border = Some(Border::from_str(&value)?),
                        "margin" => {
                            has_margin = true;
                            margin = parse_shorthand_value(&value)?;
                        }
                        "margin-top" => {
                            has_margin = true;
                            margin.top = parse_pt_value(&value)?
                        }
                        "margin-right" => {
                            has_margin = true;
                            margin.right = parse_pt_value(&value)?
                        }
                        "margin-bottom" => {
                            has_margin = true;
                            margin.bottom = parse_pt_value(&value)?
                        }
                        "margin-left" => {
                            has_margin = true;
                            margin.left = parse_pt_value(&value)?
                        }
                        "padding" => {
                            has_padding = true;
                            padding = parse_shorthand_value(&value)?;
                        }
                        "padding-top" => {
                            has_padding = true;
                            padding.top = parse_pt_value(&value)?
                        }
                        "padding-right" => {
                            has_padding = true;
                            padding.right = parse_pt_value(&value)?
                        }
                        "padding-bottom" => {
                            has_padding = true;
                            padding.bottom = parse_pt_value(&value)?
                        }
                        "padding-left" => {
                            has_padding = true;
                            padding.left = parse_pt_value(&value)?
                        }
                        "width" => style.width = Some(Dimension::Pt(parse_pt_value(&value)?)),
                        "height" => style.height = Some(Dimension::Pt(parse_pt_value(&value)?)),
                        _ => {}
                    }
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
    s.trim_end_matches("pt")
        .trim()
        .parse()
        .map_err(|_| PipelineError::TemplateParseError(format!("Invalid point value: '{}'", s)))
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
            "regular" => FontWeight::Regular,
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
            }
        }
        Err(PipelineError::TemplateParseError(format!(
            "Invalid color format: '{}'. Use #RRGGBB.",
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