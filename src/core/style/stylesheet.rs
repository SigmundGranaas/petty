use crate::parser::json::ast::StylesheetDef;
use crate::parser::style::{parse_length, parse_shorthand_margins};
use crate::parser::ParseError;
use quick_xml::events::{attributes::Attributes, Event as XmlEvent};
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::core::style::border::Border;
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::font::{FontStyle, FontWeight};
use crate::core::style::text::TextAlign;

// Main struct representing all style and layout information.
#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub page: PageLayout,
    pub styles: HashMap<String, Arc<ElementStyle>>,
    pub page_sequences: HashMap<String, PageSequence>,
}

impl Default for Stylesheet {
    fn default() -> Self {
        Stylesheet {
            page: PageLayout::default(),
            styles: HashMap::new(),
            page_sequences: HashMap::new(),
        }
    }
}

// Allow conversion from the parsed JSON stylesheet definition.
impl From<StylesheetDef> for Stylesheet {
    fn from(def: StylesheetDef) -> Self {
        Stylesheet {
            page: def.page,
            styles: def
                .styles
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            page_sequences: HashMap::new(), // Not used by JSON templates
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for PageLayout {
    fn default() -> Self {
        PageLayout {
            title: None,
            size: PageSize::A4,
            margins: Margins {
                top: 10.0,
                right: 10.0,
                bottom: 10.0,
                left: 10.0,
            },
            footer_height: 0.0,
            footer_text: None,
            footer_style: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSequence {
    pub template: String,
    pub data_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementStyle {
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default, deserialize_with = "crate::parser::style::deserialize_optional_length")]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub font_weight: Option<FontWeight>,
    #[serde(default)]
    pub font_style: Option<FontStyle>,
    #[serde(default, deserialize_with = "crate::parser::style::deserialize_optional_length")]
    pub line_height: Option<f32>,
    #[serde(default)]
    pub text_align: Option<TextAlign>,
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub margin: Option<Margins>,
    #[serde(default)]
    pub padding: Option<Margins>,
    #[serde(default)]
    pub width: Option<Dimension>,
    #[serde(default)]
    pub height: Option<Dimension>,
    #[serde(default)]
    pub background_color: Option<Color>,
    #[serde(default)]
    pub border: Option<Border>,
    #[serde(default)]
    pub border_top: Option<Border>,
    #[serde(default)]
    pub border_bottom: Option<Border>,
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

impl Stylesheet {
    /// Pre-parses an XSLT file to extract global style and layout information.
    /// NOTE: This no longer parses `xsl:attribute-set`. That is now handled by the Compiler.
    pub fn from_xslt(xslt_content: &str) -> Result<Self, ParseError> {
        let mut reader = Reader::from_str(xslt_content);
        reader.config_mut().trim_text(true);

        let mut page_layout: Option<PageLayout> = None;
        let mut page_sequences = HashMap::new();
        let mut buf = Vec::new();
        let mut in_root_template = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(e)) | Ok(XmlEvent::Empty(e)) => match e.name().as_ref() {
                    b"fo:simple-page-master" => {
                        if page_layout.is_none() {
                            page_layout = Some(parse_simple_page_master(e.attributes())?);
                        }
                    }
                    b"xsl:template" => {
                        if e.attributes()
                            .flatten()
                            .any(|a| a.key.as_ref() == b"match" && a.value.as_ref() == b"/")
                        {
                            in_root_template = true;
                        }
                    }
                    b"page-sequence" if in_root_template && page_sequences.is_empty() => {
                        let select_path = get_attr_val(e.attributes(), b"select")?
                            .unwrap_or_else(|| ".".to_string());
                        page_sequences.insert(
                            "main".to_string(), // Use a default name for the single sequence
                            PageSequence {
                                template: "main".to_string(), // Not used by XSLT engine
                                data_source: select_path,
                            },
                        );
                    }
                    _ => {}
                },
                Ok(XmlEvent::End(e)) if e.name().as_ref() == b"xsl:template" => {
                    in_root_template = false;
                }
                Ok(XmlEvent::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => (),
            }
            buf.clear();
        }

        Ok(Stylesheet {
            page: page_layout.ok_or_else(|| {
                ParseError::TemplateParse("Missing <fo:simple-page-master> tag in XSLT.".to_string())
            })?,
            styles: HashMap::new(), // Styles are now handled by the compiler.
            page_sequences,
        })
    }
}

// --- XSLT Pre-parsing Helper Functions ---

fn get_attr_val(attrs: Attributes, name: &[u8]) -> Result<Option<String>, quick_xml::Error> {
    for attr in attrs.flatten() {
        if attr.key.as_ref() == name {
            return Ok(Some(attr.unescape_value()?.into_owned()));
        }
    }
    Ok(None)
}

fn parse_simple_page_master(attrs: Attributes) -> Result<PageLayout, ParseError> {
    let mut layout = PageLayout::default();
    let mut width: Option<f32> = None;
    let mut height: Option<f32> = None;

    for attr_res in attrs {
        let attr = attr_res?;
        let value = attr.unescape_value()?;
        match attr.key.as_ref() {
            b"page-width" => width = Some(parse_length(&value)?),
            b"page-height" => height = Some(parse_length(&value)?),
            b"margin-top" => layout.margins.top = parse_length(&value)?,
            b"margin-right" => layout.margins.right = parse_length(&value)?,
            b"margin-bottom" => layout.margins.bottom = parse_length(&value)?,
            b"margin-left" => layout.margins.left = parse_length(&value)?,
            b"margin" => layout.margins = parse_shorthand_margins(&value)?,
            b"footer-text" => layout.footer_text = Some(value.into_owned()),
            b"footer-style" => layout.footer_style = Some(value.into_owned()),
            _ => {}
        }
    }

    if let (Some(w), Some(h)) = (width, height) {
        layout.size = PageSize::Custom {
            width: w,
            height: h,
        };
    } else {
        layout.size = PageSize::A4; // Default if not specified
    }

    Ok(layout)
}