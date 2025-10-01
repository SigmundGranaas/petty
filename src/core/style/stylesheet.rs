//! Defines the top-level stylesheet structure that holds all styling information.

use super::border::Border;
use super::color::Color;
use super::dimension::{Dimension, Margins, PageSize};
use super::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use super::font::{FontStyle, FontWeight};
use super::list::ListStyleType;
use super::text::TextAlign;
use crate::parser::json::ast::StylesheetDef;
use crate::parser::ParseError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageLayout {
    #[serde(default)]
    pub size: PageSize,
    #[serde(default)]
    pub margins: Option<Margins>,
    pub title: Option<String>,
    pub footer_text: Option<String>,
    pub footer_style: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementStyle {
    // Font & Text
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub line_height: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub color: Option<Color>,

    // Box Model
    pub background_color: Option<Color>,
    pub border: Option<Border>,
    pub border_top: Option<Border>,
    pub border_bottom: Option<Border>,
    pub margin: Option<Margins>,
    pub padding: Option<Margins>,
    pub width: Option<Dimension>,
    pub height: Option<Dimension>,

    // List Properties
    pub list_style_type: Option<ListStyleType>,

    // Flexbox Container
    pub flex_direction: Option<FlexDirection>,
    pub flex_wrap: Option<FlexWrap>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,

    // Flexbox Item
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub flex_basis: Option<Dimension>,
    pub align_self: Option<AlignSelf>,
}