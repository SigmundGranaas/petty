use crate::parser::json::ast::StylesheetDef;
use crate::parser::style::{parse_border, parse_color, parse_dimension, parse_length, parse_shorthand_margins};
use crate::parser::ParseError;
use quick_xml::events::{attributes::Attributes, Event as XmlEvent};
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
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

#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
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

impl fmt::Debug for ElementStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("ElementStyle");
        if let Some(v) = &self.font_family { dbg.field("font_family", v); }
        if let Some(v) = self.font_size { dbg.field("font_size", &v); }
        if let Some(v) = &self.font_weight { dbg.field("font_weight", v); }
        if let Some(v) = &self.font_style { dbg.field("font_style", v); }
        if let Some(v) = self.line_height { dbg.field("line_height", &v); }
        if let Some(v) = &self.text_align { dbg.field("text_align", v); }
        if let Some(v) = &self.color { dbg.field("color", v); }
        if let Some(v) = &self.margin { dbg.field("margin", v); }
        if let Some(v) = &self.padding { dbg.field("padding", v); }
        if let Some(v) = &self.width { dbg.field("width", v); }
        if let Some(v) = &self.height { dbg.field("height", v); }
        if let Some(v) = &self.background_color { dbg.field("background_color", v); }
        if let Some(v) = &self.border { dbg.field("border", v); }
        if let Some(v) = &self.border_top { dbg.field("border_top", v); }
        if let Some(v) = &self.border_bottom { dbg.field("border_bottom", v); }
        dbg.finish()
    }
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
    pub fn from_xslt(xslt_content: &str) -> Result<Self, ParseError> {
        let mut reader = Reader::from_str(xslt_content);
        reader.config_mut().trim_text(true);

        let mut page_layout: Option<PageLayout> = None;
        let mut styles = HashMap::new();
        let mut page_sequences = HashMap::new();
        let mut buf = Vec::new();
        let mut in_root_template = false;

        // State for parsing attribute-sets
        let mut current_style_name: Option<String> = None;
        let mut current_style: Option<ElementStyle> = None;
        let mut current_attr_name: Option<String> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(e)) => match e.name().as_ref() {
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
                            "main".to_string(),
                            PageSequence {
                                template: "main".to_string(),
                                data_source: select_path,
                            },
                        );
                    }
                    b"xsl:attribute-set" => {
                        if let Some(name) = get_attr_val(e.attributes(), b"name")? {
                            current_style_name = Some(name);
                            current_style = Some(ElementStyle::default());
                        }
                    }
                    b"xsl:attribute" => {
                        if current_style.is_some() {
                            if let Some(name) = get_attr_val(e.attributes(), b"name")? {
                                current_attr_name = Some(name);
                            }
                        }
                    }
                    _ => {}
                },
                Ok(XmlEvent::Empty(e)) => match e.name().as_ref() {
                    b"fo:simple-page-master" => {
                        if page_layout.is_none() {
                            page_layout = Some(parse_simple_page_master(e.attributes())?);
                        }
                    }
                    _ => {}
                },
                Ok(XmlEvent::Text(e)) => {
                    if let (Some(style), Some(attr_name)) = (current_style.as_mut(), current_attr_name.take()) {
                        let value = e.unescape()?.to_string();
                        parse_and_apply_style_attribute(style, &attr_name, &value)
                            .map_err(|err| ParseError::TemplateParse(format!("Failed to parse style attribute '{}: {}': {}", attr_name, value, err)))?;
                    }
                },
                Ok(XmlEvent::End(e)) => match e.name().as_ref() {
                    b"xsl:template" => {
                        in_root_template = false;
                    }
                    b"xsl:attribute-set" => {
                        if let (Some(name), Some(style)) = (current_style_name.take(), current_style.take()) {
                            styles.insert(name, Arc::new(style));
                        }
                    }
                    b"xsl:attribute" => {
                        current_attr_name = None;
                    }
                    _ => (),
                },
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
            styles, // Use the newly parsed styles
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

/// Helper to parse a single XSLT style attribute and apply it to an ElementStyle.
fn parse_and_apply_style_attribute(style: &mut ElementStyle, name: &str, value: &str) -> Result<(), ParseError> {
    match name {
        "font-family" => style.font_family = Some(value.to_string()),
        "font-size" => style.font_size = Some(parse_length(value)?),
        "font-weight" => style.font_weight = Some(match value.to_lowercase().as_str() {
            "bold" => FontWeight::Bold,
            "normal" | "regular" => FontWeight::Regular,
            "light" => FontWeight::Light,
            "thin" => FontWeight::Thin,
            "medium" => FontWeight::Medium,
            "black" => FontWeight::Black,
            _ => return Err(ParseError::TemplateParse(format!("Unknown font-weight: {}", value))),
        }),
        "font-style" => style.font_style = Some(match value.to_lowercase().as_str() {
            "normal" => FontStyle::Normal,
            "italic" => FontStyle::Italic,
            "oblique" => FontStyle::Oblique,
            _ => return Err(ParseError::TemplateParse(format!("Unknown font-style: {}", value))),
        }),
        "line-height" => style.line_height = Some(parse_length(value)?),
        "color" => style.color = Some(parse_color(value)?),
        "background-color" => style.background_color = Some(parse_color(value)?),
        "text-align" => style.text_align = Some(match value.to_lowercase().as_str() {
            "left" => TextAlign::Left,
            "right" => TextAlign::Right,
            "center" => TextAlign::Center,
            "justify" => TextAlign::Justify,
            _ => return Err(ParseError::TemplateParse(format!("Unknown text-align: {}", value))),
        }),
        "width" => style.width = Some(parse_dimension(value)?),
        "height" => style.height = Some(parse_dimension(value)?),
        "margin" => style.margin = Some(parse_shorthand_margins(value)?),
        "margin-top" => style.margin.get_or_insert_with(Default::default).top = parse_length(value)?,
        "margin-right" => style.margin.get_or_insert_with(Default::default).right = parse_length(value)?,
        "margin-bottom" => style.margin.get_or_insert_with(Default::default).bottom = parse_length(value)?,
        "margin-left" => style.margin.get_or_insert_with(Default::default).left = parse_length(value)?,
        "padding" => style.padding = Some(parse_shorthand_margins(value)?),
        "padding-top" => style.padding.get_or_insert_with(Default::default).top = parse_length(value)?,
        "padding-right" => style.padding.get_or_insert_with(Default::default).right = parse_length(value)?,
        "padding-bottom" => style.padding.get_or_insert_with(Default::default).bottom = parse_length(value)?,
        "padding-left" => style.padding.get_or_insert_with(Default::default).left = parse_length(value)?,
        "border" => style.border = Some(parse_border(value)?),
        "border-top" => style.border_top = Some(parse_border(value)?),
        "border-bottom" => style.border_bottom = Some(parse_border(value)?),
        _ => log::warn!("Unsupported XSLT style attribute: '{}'", name),
    }
    Ok(())
}