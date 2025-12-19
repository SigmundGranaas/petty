//! Centralized parsing for all style-related attributes and properties.
//!
//! This module provides high-level functions that act as a facade, using the robust
//! `nom` parsers from `petty_style::parsers` to interpret strings from template
//! files into the strongly-typed style primitives defined in `crate::core`.

use crate::style_types::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use crate::style_types::list::ListStyleType;
use petty_style::parsers::{run_parser, parse_dimension, parse_length, parse_color, parse_border, parse_shorthand_margins};
use crate::parser::ParseError;
use crate::style_types::font::{FontStyle, FontWeight};
use crate::style_types::stylesheet::ElementStyle;
use crate::style_types::text::TextAlign;
use crate::style_types::dimension::PageSize;

// --- High-level Parsers (Facades over `style_parsers`) ---

/// Parses a font weight string (e.g., "bold", "400").
pub fn parse_font_weight(s: &str) -> Result<FontWeight, ParseError> {
    match s.to_lowercase().as_str() {
        "thin" => Ok(FontWeight::Thin),
        "light" => Ok(FontWeight::Light),
        "regular" | "normal" => Ok(FontWeight::Regular),
        "medium" => Ok(FontWeight::Medium),
        "bold" => Ok(FontWeight::Bold),
        "black" => Ok(FontWeight::Black),
        _ => {
            let num_weight = s.parse::<u16>()
                .map_err(|_| ParseError::TemplateParse(format!("Invalid font weight: '{}'", s)))?;
            Ok(FontWeight::Numeric(num_weight))
        }
    }
}

pub fn parse_font_style(s: &str) -> Result<FontStyle, ParseError> {
    match s.to_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(ParseError::TemplateParse(format!("Invalid font style: {}", s))),
    }
}

pub fn parse_text_align(s: &str) -> Result<TextAlign, ParseError> {
    match s.to_lowercase().as_str() {
        "left" => Ok(TextAlign::Left),
        "right" => Ok(TextAlign::Right),
        "center" => Ok(TextAlign::Center),
        "justify" => Ok(TextAlign::Justify),
        _ => Err(ParseError::TemplateParse(format!("Invalid text align: {}", s))),
    }
}

pub fn parse_list_style_type(s: &str) -> Result<ListStyleType, ParseError> {
    match s.to_lowercase().as_str() {
        "disc" => Ok(ListStyleType::Disc),
        "circle" => Ok(ListStyleType::Circle),
        "square" => Ok(ListStyleType::Square),
        "decimal" => Ok(ListStyleType::Decimal),
        "none" => Ok(ListStyleType::None),
        _ => Err(ParseError::TemplateParse(format!("Invalid list-style-type: {}", s))),
    }
}

pub fn parse_flex_direction(s: &str) -> Result<FlexDirection, ParseError> {
    match s.to_lowercase().as_str() {
        "row" => Ok(FlexDirection::Row),
        "row-reverse" => Ok(FlexDirection::RowReverse),
        "column" => Ok(FlexDirection::Column),
        "column-reverse" => Ok(FlexDirection::ColumnReverse),
        _ => Err(ParseError::TemplateParse(format!("Invalid flex-direction: {}", s))),
    }
}

pub fn parse_flex_wrap(s: &str) -> Result<FlexWrap, ParseError> {
    match s.to_lowercase().as_str() {
        "nowrap" => Ok(FlexWrap::NoWrap),
        "wrap" => Ok(FlexWrap::Wrap),
        "wrap-reverse" => Ok(FlexWrap::WrapReverse),
        _ => Err(ParseError::TemplateParse(format!("Invalid flex-wrap: {}", s))),
    }
}

pub fn parse_justify_content(s: &str) -> Result<JustifyContent, ParseError> {
    match s.to_lowercase().as_str() {
        "flex-start" => Ok(JustifyContent::FlexStart),
        "flex-end" => Ok(JustifyContent::FlexEnd),
        "center" => Ok(JustifyContent::Center),
        "space-between" => Ok(JustifyContent::SpaceBetween),
        "space-around" => Ok(JustifyContent::SpaceAround),
        "space-evenly" => Ok(JustifyContent::SpaceEvenly),
        _ => Err(ParseError::TemplateParse(format!("Invalid justify-content: {}", s))),
    }
}

pub fn parse_align_items(s: &str) -> Result<AlignItems, ParseError> {
    match s.to_lowercase().as_str() {
        "stretch" => Ok(AlignItems::Stretch),
        "flex-start" => Ok(AlignItems::FlexStart),
        "flex-end" => Ok(AlignItems::FlexEnd),
        "center" => Ok(AlignItems::Center),
        "baseline" => Ok(AlignItems::Baseline),
        _ => Err(ParseError::TemplateParse(format!("Invalid align-items: {}", s))),
    }
}

pub fn parse_align_self(s: &str) -> Result<AlignSelf, ParseError> {
    match s.to_lowercase().as_str() {
        "auto" => Ok(AlignSelf::Auto),
        "stretch" => Ok(AlignSelf::Stretch),
        "flex-start" => Ok(AlignSelf::FlexStart),
        "flex-end" => Ok(AlignSelf::FlexEnd),
        "center" => Ok(AlignSelf::Center),
        "baseline" => Ok(AlignSelf::Baseline),
        _ => Err(ParseError::TemplateParse(format!("Invalid align-self: {}", s))),
    }
}

pub fn parse_page_size(s: &str) -> Result<PageSize, ParseError> {
    match s.to_lowercase().as_str() {
        "a4" => Ok(PageSize::A4),
        "letter" => Ok(PageSize::Letter),
        "legal" => Ok(PageSize::Legal),
        _ => Err(ParseError::TemplateParse(format!("Unknown page size: {}", s))),
    }
}

// --- High-level Parsers for XSLT ---

/// Applies a single parsed style property to an `ElementStyle` struct.
/// This is the central dispatcher for applying individual CSS-like properties.
pub fn apply_style_property(style: &mut ElementStyle, attr_name: &str, value: &str) -> Result<(), ParseError> {
    match attr_name {
        "font-family" => style.font_family = Some(value.to_string()),
        "font-size" => style.font_size = Some(run_parser(parse_length, value)?),
        "font-weight" => style.font_weight = Some(parse_font_weight(value)?),
        "font-style" => style.font_style = Some(parse_font_style(value)?),
        "line-height" => style.line_height = Some(run_parser(parse_length, value)?),
        "text-align" => style.text_align = Some(parse_text_align(value)?),
        "color" => style.color = Some(run_parser(parse_color, value)?),
        "background-color" => style.background_color = Some(run_parser(parse_color, value)?),
        "border" => style.border = Some(run_parser(parse_border, value)?),
        "border-top" => style.border_top = Some(run_parser(parse_border, value)?),
        "border-bottom" => style.border_bottom = Some(run_parser(parse_border, value)?),
        "margin" => style.margin = Some(parse_shorthand_margins(value)?),
        "margin-top" => style.margin.get_or_insert_with(Default::default).top = run_parser(parse_length, value)?,
        "margin-right" => style.margin.get_or_insert_with(Default::default).right = run_parser(parse_length, value)?,
        "margin-bottom" => style.margin.get_or_insert_with(Default::default).bottom = run_parser(parse_length, value)?,
        "margin-left" => style.margin.get_or_insert_with(Default::default).left = run_parser(parse_length, value)?,
        "padding" => style.padding = Some(parse_shorthand_margins(value)?),
        "padding-top" => style.padding.get_or_insert_with(Default::default).top = run_parser(parse_length, value)?,
        "padding-right" => style.padding.get_or_insert_with(Default::default).right = run_parser(parse_length, value)?,
        "padding-bottom" => style.padding.get_or_insert_with(Default::default).bottom = run_parser(parse_length, value)?,
        "padding-left" => style.padding.get_or_insert_with(Default::default).left = run_parser(parse_length, value)?,
        "width" => style.width = Some(run_parser(parse_dimension, value)?),
        "height" => style.height = Some(run_parser(parse_dimension, value)?),
        "list-style-type" => style.list_style_type = Some(parse_list_style_type(value)?),
        "flex-direction" => style.flex_direction = Some(parse_flex_direction(value)?),
        "flex-wrap" => style.flex_wrap = Some(parse_flex_wrap(value)?),
        "justify-content" => style.justify_content = Some(parse_justify_content(value)?),
        "align-items" => style.align_items = Some(parse_align_items(value)?),
        "flex-grow" => style.flex_grow = Some(value.trim().parse::<f32>().map_err(|e| ParseError::FloatParse(e, value.to_string()))?),
        "flex-shrink" => style.flex_shrink = Some(value.trim().parse::<f32>().map_err(|e| ParseError::FloatParse(e, value.to_string()))?),
        "flex-basis" => style.flex_basis = Some(run_parser(parse_dimension, value)?),
        "align-self" => style.align_self = Some(parse_align_self(value)?),
        _ => {} // Not a style attribute, ignore.
    };
    Ok(())
}

/// Parses XSL-FO attributes on an XML tag into an `ElementStyle` object.
pub fn parse_fo_attributes(attrs: &[(Vec<u8>, Vec<u8>)], style_override: &mut ElementStyle) -> Result<(), ParseError> {
    for (key, value) in attrs {
        let key_str = String::from_utf8_lossy(key);
        let value_str = String::from_utf8_lossy(value);
        if key_str == "style" || key_str == "use-attribute-sets" { continue; }
        apply_style_property(style_override, &key_str, &value_str)?;
    }
    Ok(())
}

/// Parses an inline `style="key: value; ..."` attribute.
pub fn parse_inline_css(css: &str, style_override: &mut ElementStyle) -> Result<(), ParseError> {
    for declaration in css.split(';') {
        if let Some((key, value)) = declaration.split_once(':') {
            apply_style_property(style_override, key.trim(), value.trim())?;
        }
    }
    Ok(())
}