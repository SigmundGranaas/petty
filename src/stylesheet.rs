use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Stylesheet {
    pub page: PageLayout,
    pub styles: HashMap<String, ElementStyle>,
    #[serde(default)]
    pub templates: HashMap<String, Template>,
    #[serde(default)]
    pub page_sequences: HashMap<String, PageSequence>,
    #[serde(default)]
    pub rules: Vec<StyleRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSequence {
    pub template: String,
    pub data_source: String, // JSON pointer to an array of objects
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageLayout {
    pub size: PageSize,
    pub margins: Margins,
    pub header: Option<HeaderFooter>,
    pub footer: Option<HeaderFooter>,
    pub columns: u32,
    pub column_gap: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PageSize {
    A4, Letter, Legal,
    Custom { width: f32, height: f32 },
}

impl Default for PageSize {
    fn default() -> Self { PageSize::A4 }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
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
    pub display: Option<Display>,
    pub border: Option<Border>,
    pub border_radius: Option<f32>,
    pub background_color: Option<Color>,
    pub background_image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub children: Vec<TemplateElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TemplateElement {
    Text { content: String, style: Option<String> },
    Image { src: String, alt: Option<String>, style: Option<String> },
    Table { data_source: String, columns: Vec<TableColumn>, style: Option<String> },
    Container { children: Vec<TemplateElement>, style: Option<String> },
    Rectangle { style: Option<String> },
    PageBreak,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColumn {
    pub header: String,
    pub data_field: String, // JSON pointer relative to row data
    pub width: Option<Dimension>,
    pub style: Option<String>,
    pub header_style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleRule {
    pub selector: String,
    pub style: ElementStyle,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dimension { Px(f32), Pt(f32), Percent(f32), Auto }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontWeight { Thin, Light, Regular, Medium, Bold, Black, #[serde(untagged)] Numeric(u16) }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontStyle { Normal, Italic, Oblique }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TextAlign { Left, Right, Center, Justify }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Display { Block, Inline, InlineBlock, Table, TableRow, TableCell, None }

// Helper function to provide a default alpha value of 1.0 (fully opaque)
fn default_alpha() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: f32
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Border { pub width: f32, pub style: BorderStyle, pub color: Color }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BorderStyle { Solid, Dashed, Dotted, Double, None }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderFooter { pub height: f32, pub children: Vec<TemplateElement>, pub style: Option<String> }

impl Stylesheet {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}