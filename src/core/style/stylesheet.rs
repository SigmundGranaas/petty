//! Defines the top-level stylesheet structure that holds all styling information.

use super::border::Border;
use super::color::Color;
use super::dimension::{Dimension, Margins, PageSize};
use super::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use super::font::{FontStyle, FontWeight};
use super::list::{ListStylePosition, ListStyleType};
use super::text::{TextAlign, TextDecoration};
use crate::parser::json::ast::StylesheetDef;
use crate::parser::ParseError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    /// A map of all named page layouts defined in the template.
    pub page_masters: HashMap<String, PageLayout>,
    /// The name of the master to use for the first page.
    pub default_page_master_name: Option<String>,
    /// A map of all named element styles.
    pub styles: HashMap<String, Arc<ElementStyle>>,
}

impl From<StylesheetDef> for Stylesheet {
    fn from(def: StylesheetDef) -> Self {
        let default_page_master_name = def.page_masters.keys().next().cloned();
        Self {
            page_masters: def.page_masters,
            default_page_master_name,
            styles: def.styles.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
        }
    }
}

impl Stylesheet {
    /// Creates a new `Stylesheet` by parsing a raw XSLT string.
    /// This is the primary entry point for XSLT-based styling.
    pub fn from_xslt(xslt_content: &str) -> Result<Self, ParseError> {
        crate::parser::stylesheet_parser::XsltParser::new(xslt_content).parse()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageLayout {
    #[serde(default)]
    pub size: PageSize,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margins: Option<Margins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer_style: Option<String>,
}

#[derive(Deserialize, Serialize, Default, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementStyle {
    // Font & Text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<FontWeight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_style: Option<FontStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<TextAlign>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_decoration: Option<TextDecoration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orphans: Option<usize>,

    // Box Model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<Margins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<Margins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<Dimension>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<Dimension>,

    // List Properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_style_type: Option<ListStyleType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_style_position: Option<ListStylePosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_style_image: Option<String>,

    // Table Properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_spacing: Option<f32>,

    // Flexbox Container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_direction: Option<FlexDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_wrap: Option<FlexWrap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify_content: Option<JustifyContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_items: Option<AlignItems>,

    // Flexbox Item
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_grow: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_shrink: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_basis: Option<Dimension>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_self: Option<AlignSelf>,
}

impl fmt::Debug for ElementStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("ElementStyle");
        if let Some(val) = &self.font_family { dbg.field("font_family", val); }
        if let Some(val) = &self.font_size { dbg.field("font_size", val); }
        if let Some(val) = &self.font_weight { dbg.field("font_weight", val); }
        if let Some(val) = &self.font_style { dbg.field("font_style", val); }
        if let Some(val) = &self.line_height { dbg.field("line_height", val); }
        if let Some(val) = &self.text_align { dbg.field("text_align", val); }
        if let Some(val) = &self.color { dbg.field("color", val); }
        if let Some(val) = &self.text_decoration { dbg.field("text_decoration", val); }
        if let Some(val) = &self.widows { dbg.field("widows", val); }
        if let Some(val) = &self.orphans { dbg.field("orphans", val); }
        if let Some(val) = &self.background_color { dbg.field("background_color", val); }
        if let Some(val) = &self.border { dbg.field("border", val); }
        if let Some(val) = &self.border_top { dbg.field("border_top", val); }
        if let Some(val) = &self.border_right { dbg.field("border_right", val); }
        if let Some(val) = &self.border_bottom { dbg.field("border_bottom", val); }
        if let Some(val) = &self.border_left { dbg.field("border_left", val); }
        if let Some(val) = &self.margin { dbg.field("margin", val); }
        if let Some(val) = &self.padding { dbg.field("padding", val); }
        if let Some(val) = &self.width { dbg.field("width", val); }
        if let Some(val) = &self.height { dbg.field("height", val); }
        if let Some(val) = &self.list_style_type { dbg.field("list_style_type", val); }
        if let Some(val) = &self.list_style_position { dbg.field("list_style_position", val); }
        if let Some(val) = &self.list_style_image { dbg.field("list_style_image", val); }
        if let Some(val) = &self.border_spacing { dbg.field("border_spacing", val); }
        if let Some(val) = &self.flex_direction { dbg.field("flex_direction", val); }
        if let Some(val) = &self.flex_wrap { dbg.field("flex_wrap", val); }
        if let Some(val) = &self.justify_content { dbg.field("justify_content", val); }
        if let Some(val) = &self.align_items { dbg.field("align_items", val); }
        if let Some(val) = &self.order { dbg.field("order", val); }
        if let Some(val) = &self.flex_grow { dbg.field("flex_grow", val); }
        if let Some(val) = &self.flex_shrink { dbg.field("flex_shrink", val); }
        if let Some(val) = &self.flex_basis { dbg.field("flex_basis", val); }
        if let Some(val) = &self.align_self { dbg.field("align_self", val); }
        dbg.finish()
    }
}