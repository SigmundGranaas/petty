//! Centralized parsing for all style-related attributes and properties.
//!
//! This module provides functions to parse strings from template files (both XSLT and JSON)
//! into the strongly-typed style primitives defined in `crate::core`. It decouples the
//! core parsers from the specifics of CSS-like value parsing.

use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use crate::core::style::list::ListStyleType;
use crate::parser::ParseError;
use serde::{de, Deserialize, Deserializer};
use crate::core::style::border::{Border, BorderStyle};
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::font::{FontStyle, FontWeight};
use crate::core::style::stylesheet::ElementStyle;
use crate::core::style::text::TextAlign;
// --- Length & Dimension Parsing ---

/// Parses a length attribute string (e.g., "12pt", "20mm") into a single f32 value in points.
pub fn parse_length(s: &str) -> Result<f32, ParseError> {
    let s_trimmed = s.trim();
    let parse_with_context = |val_str: &str| -> Result<f32, ParseError> {
        val_str
            .trim()
            .parse::<f32>()
            .map_err(|e| ParseError::FloatParse(e, s.to_string()))
    };

    if let Some(val_str) = s_trimmed.strip_suffix("pt") {
        Ok(parse_with_context(val_str)?)
    } else if let Some(val_str) = s_trimmed.strip_suffix("px") {
        Ok(parse_with_context(val_str)?) // Treat px as pt for PDF context
    } else if let Some(val_str) = s_trimmed.strip_suffix("in") {
        Ok(parse_with_context(val_str)? * 72.0)
    } else if let Some(val_str) = s_trimmed.strip_suffix("cm") {
        Ok(parse_with_context(val_str)? * 28.35)
    } else if let Some(val_str) = s_trimmed.strip_suffix("mm") {
        Ok(parse_with_context(val_str)? * 2.835)
    } else {
        // Assume points if no unit is specified
        Ok(parse_with_context(s_trimmed)?)
    }
}

/// Parses a string like "50%", "120pt", or "auto" into a Dimension enum.
pub fn parse_dimension(s: &str) -> Result<Dimension, ParseError> {
    let s_trimmed = s.trim();
    if s_trimmed == "auto" {
        return Ok(Dimension::Auto);
    }
    if s_trimmed.ends_with('%') {
        let val_str = s_trimmed.trim_end_matches('%');
        let val = val_str
            .parse::<f32>()
            .map_err(|e| ParseError::FloatParse(e, s.to_string()))?;
        Ok(Dimension::Percent(val))
    } else {
        Ok(Dimension::Pt(parse_length(s_trimmed)?))
    }
}

// --- Shorthand Properties ---

/// Parses CSS-style shorthand for margin/padding, supporting all units.
pub fn parse_shorthand_margins(s: &str) -> Result<Margins, ParseError> {
    let parts: Vec<f32> = s
        .split_whitespace()
        .map(parse_length)
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
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid shorthand value count: '{}'",
            s
        ))),
    }
}

// --- Color & Border Parsing ---

/// Parses a color string from hex format (e.g., "#RRGGBB" or "#RGB").
pub fn parse_color(s: &str) -> Result<Color, ParseError> {
    if s.starts_with('#') {
        let hex = s.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| {
                ParseError::TemplateParse(format!("Invalid hex value in color '{}'", s))
            })?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| {
                ParseError::TemplateParse(format!("Invalid hex value in color '{}'", s))
            })?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| {
                ParseError::TemplateParse(format!("Invalid hex value in color '{}'", s))
            })?;
            return Ok(Color { r, g, b, a: 1.0 });
        } else if hex.len() == 3 {
            let r_char = hex.chars().next().unwrap();
            let g_char = hex.chars().nth(1).unwrap();
            let b_char = hex.chars().nth(2).unwrap();
            let r = u8::from_str_radix(&format!("{}{}", r_char, r_char), 16).map_err(|_| {
                ParseError::TemplateParse(format!("Invalid hex value in color '{}'", s))
            })?;
            let g = u8::from_str_radix(&format!("{}{}", g_char, g_char), 16).map_err(|_| {
                ParseError::TemplateParse(format!("Invalid hex value in color '{}'", s))
            })?;
            let b = u8::from_str_radix(&format!("{}{}", b_char, b_char), 16).map_err(|_| {
                ParseError::TemplateParse(format!("Invalid hex value in color '{}'", s))
            })?;
            return Ok(Color { r, g, b, a: 1.0 });
        }
    }
    Err(ParseError::TemplateParse(format!(
        "Invalid color format: '{}'. Use #RRGGBB or #RGB.",
        s
    )))
}

/// Parses a border style string (e.g., "solid", "dashed").
pub fn parse_border_style(s: &str) -> Result<BorderStyle, ParseError> {
    match s.to_lowercase().as_str() {
        "solid" => Ok(BorderStyle::Solid),
        "dashed" => Ok(BorderStyle::Dashed),
        "dotted" => Ok(BorderStyle::Dotted),
        "double" => Ok(BorderStyle::Double),
        "none" => Ok(BorderStyle::None),
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid border style: {}",
            s
        ))),
    }
}

/// Parses a border shorthand string (e.g., "1pt solid #000000").
pub fn parse_border(s: &str) -> Result<Border, ParseError> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(ParseError::TemplateParse(format!(
            "Invalid border format: '{}'. Use 'width style #RRGGBB'.",
            s
        )));
    }
    Ok(Border {
        width: parse_length(parts[0])?,
        style: parse_border_style(parts[1])?,
        color: parse_color(parts[2])?,
    })
}

// --- Font & Text Parsing ---

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
            let num_weight = s
                .parse::<u16>()
                .map_err(|_| ParseError::TemplateParse(format!("Invalid font weight: '{}'", s)))?;
            Ok(FontWeight::Numeric(num_weight))
        }
    }
}

impl<'de> Deserialize<'de> for FontWeight {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum FontWeightDef {
            Str(String),
            Num(u16),
        }

        match FontWeightDef::deserialize(deserializer)? {
            FontWeightDef::Str(s) => parse_font_weight(&s).map_err(de::Error::custom),
            FontWeightDef::Num(n) => Ok(FontWeight::Numeric(n)),
        }
    }
}


/// Parses a font style string (e.g., "italic").
pub fn parse_font_style(s: &str) -> Result<FontStyle, ParseError> {
    match s.to_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid font style: {}",
            s
        ))),
    }
}

/// Parses a text alignment string (e.g., "center").
pub fn parse_text_align(s: &str) -> Result<TextAlign, ParseError> {
    match s.to_lowercase().as_str() {
        "left" => Ok(TextAlign::Left),
        "right" => Ok(TextAlign::Right),
        "center" => Ok(TextAlign::Center),
        "justify" => Ok(TextAlign::Justify),
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid text align: {}",
            s
        ))),
    }
}

// --- List Properties ---
pub fn parse_list_style_type(s: &str) -> Result<ListStyleType, ParseError> {
    match s.to_lowercase().as_str() {
        "disc" => Ok(ListStyleType::Disc),
        "circle" => Ok(ListStyleType::Circle),
        "square" => Ok(ListStyleType::Square),
        "decimal" => Ok(ListStyleType::Decimal),
        "none" => Ok(ListStyleType::None),
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid list-style-type: {}",
            s
        ))),
    }
}

// --- Flexbox Properties ---
pub fn parse_flex_direction(s: &str) -> Result<FlexDirection, ParseError> {
    match s.to_lowercase().as_str() {
        "row" => Ok(FlexDirection::Row),
        "row-reverse" => Ok(FlexDirection::RowReverse),
        "column" => Ok(FlexDirection::Column),
        "column-reverse" => Ok(FlexDirection::ColumnReverse),
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid flex-direction: {}",
            s
        ))),
    }
}

pub fn parse_flex_wrap(s: &str) -> Result<FlexWrap, ParseError> {
    match s.to_lowercase().as_str() {
        "nowrap" => Ok(FlexWrap::NoWrap),
        "wrap" => Ok(FlexWrap::Wrap),
        "wrap-reverse" => Ok(FlexWrap::WrapReverse),
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid flex-wrap: {}",
            s
        ))),
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
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid justify-content: {}",
            s
        ))),
    }
}

pub fn parse_align_items(s: &str) -> Result<AlignItems, ParseError> {
    match s.to_lowercase().as_str() {
        "stretch" => Ok(AlignItems::Stretch),
        "flex-start" => Ok(AlignItems::FlexStart),
        "flex-end" => Ok(AlignItems::FlexEnd),
        "center" => Ok(AlignItems::Center),
        "baseline" => Ok(AlignItems::Baseline),
        _ => Err(ParseError::TemplateParse(format!(
            "Invalid align-items: {}",
            s
        ))),
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

// --- Page Layout Parsing ---

/// Parses a page size string (e.g., "A4", "Letter").
pub fn parse_page_size(s: &str) -> Result<PageSize, ParseError> {
    match s.to_lowercase().as_str() {
        "a4" => Ok(PageSize::A4),
        "letter" => Ok(PageSize::Letter),
        "legal" => Ok(PageSize::Legal),
        _ => Err(ParseError::TemplateParse(format!(
            "Unknown page size: {}",
            s
        ))),
    }
}

// --- High-level Parsers for XSLT ---

/// Applies a single parsed style property to an `ElementStyle` struct.
/// This is the central dispatcher for applying individual CSS-like properties.
pub fn apply_style_property(
    style: &mut ElementStyle,
    attr_name: &str,
    value: &str,
) -> Result<(), ParseError> {
    match attr_name {
        "font-family" => style.font_family = Some(value.to_string()),
        "font-size" => style.font_size = Some(parse_length(value)?),
        "font-weight" => style.font_weight = Some(parse_font_weight(value)?),
        "font-style" => style.font_style = Some(parse_font_style(value)?),
        "line-height" => style.line_height = Some(parse_length(value)?),
        "text-align" => style.text_align = Some(parse_text_align(value)?),
        "color" => style.color = Some(parse_color(value)?),
        "background-color" => style.background_color = Some(parse_color(value)?),
        "border" => style.border = Some(parse_border(value)?),
        "border-top" => style.border_top = Some(parse_border(value)?),
        "border-bottom" => style.border_bottom = Some(parse_border(value)?),
        "margin" => style.margin = Some(parse_shorthand_margins(value)?),
        "margin-top" => style.margin.get_or_insert_with(Default::default).top = parse_length(value)?,
        "margin-right" => {
            style.margin.get_or_insert_with(Default::default).right = parse_length(value)?
        }
        "margin-bottom" => {
            style.margin.get_or_insert_with(Default::default).bottom = parse_length(value)?
        }
        "margin-left" => {
            style.margin.get_or_insert_with(Default::default).left = parse_length(value)?
        }
        "padding" => style.padding = Some(parse_shorthand_margins(value)?),
        "padding-top" => {
            style.padding.get_or_insert_with(Default::default).top = parse_length(value)?
        }
        "padding-right" => {
            style.padding.get_or_insert_with(Default::default).right = parse_length(value)?
        }
        "padding-bottom" => {
            style.padding.get_or_insert_with(Default::default).bottom = parse_length(value)?
        }
        "padding-left" => {
            style.padding.get_or_insert_with(Default::default).left = parse_length(value)?
        }
        "width" => style.width = Some(parse_dimension(value)?),
        "height" => style.height = Some(parse_dimension(value)?),

        // List Properties
        "list-style-type" => style.list_style_type = Some(parse_list_style_type(value)?),

        // Flexbox Container Properties
        "flex-direction" => style.flex_direction = Some(parse_flex_direction(value)?),
        "flex-wrap" => style.flex_wrap = Some(parse_flex_wrap(value)?),
        "justify-content" => style.justify_content = Some(parse_justify_content(value)?),
        "align-items" => style.align_items = Some(parse_align_items(value)?),

        // Flexbox Item Properties
        "flex-grow" => {
            style.flex_grow = Some(
                value
                    .trim()
                    .parse::<f32>()
                    .map_err(|e| ParseError::FloatParse(e, value.to_string()))?,
            )
        }
        "flex-shrink" => {
            style.flex_shrink = Some(
                value
                    .trim()
                    .parse::<f32>()
                    .map_err(|e| ParseError::FloatParse(e, value.to_string()))?,
            )
        }
        "flex-basis" => style.flex_basis = Some(parse_dimension(value)?),
        "align-self" => style.align_self = Some(parse_align_self(value)?),

        _ => {} // Not a style attribute, just ignore.
    };
    Ok(())
}

/// Parses XSL-FO attributes on an XML tag into an `ElementStyle` object.
pub fn parse_fo_attributes(
    attributes: &[(Vec<u8>, Vec<u8>)],
    style_override: &mut ElementStyle,
) -> Result<(), ParseError> {
    for (key, value) in attributes {
        let key_str = String::from_utf8_lossy(key);
        let value_str = String::from_utf8_lossy(value);
        if key_str == "style" || key_str == "use-attribute-sets" {
            continue;
        }
        apply_style_property(style_override, &key_str, &value_str)?;
    }
    Ok(())
}

/// Parses an inline `style="key: value; ..."` attribute.
pub fn parse_inline_css(
    css: &str,
    style_override: &mut ElementStyle,
) -> Result<(), ParseError> {
    for declaration in css.split(';') {
        if let Some((key, value)) = declaration.split_once(':') {
            apply_style_property(style_override, key.trim(), value.trim())?;
        }
    }
    Ok(())
}

/// A custom deserializer for `Option<f32>` that can handle strings with units (e.g., "12pt").
pub fn deserialize_optional_length<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrFloat {
        Str(String),
        Num(f32),
    }

    match Option::<StringOrFloat>::deserialize(deserializer)? {
        Some(StringOrFloat::Str(s)) => parse_length(&s).map(Some).map_err(|e| {
            let err_str = match e {
                ParseError::FloatParse(_, val) => format!("invalid number in length value: '{}'", val),
                _ => "invalid length format".to_string(),
            };
            de::Error::custom(err_str)
        }),
        Some(StringOrFloat::Num(n)) => Ok(Some(n)),
        None => Ok(None),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::style::parse_length;
    use serde::Deserialize;
    use crate::core::style::dimension::PageSize;

    #[test]
    fn test_parse_length() {
        assert_eq!(parse_length("12pt").unwrap(), 12.0);
        assert_eq!(parse_length("12px").unwrap(), 12.0);
        assert_eq!(parse_length(" 1in ").unwrap(), 72.0);
        assert_eq!(parse_length("10mm").unwrap(), 28.35);
        assert_eq!(parse_length("1cm").unwrap(), 28.35);
        assert_eq!(parse_length("10").unwrap(), 10.0);
        assert!(parse_length("abc").is_err());
    }

    #[test]
    fn test_parse_dimension() {
        assert_eq!(parse_dimension("12pt").unwrap(), Dimension::Pt(12.0));
        assert_eq!(parse_dimension("50%").unwrap(), Dimension::Percent(50.0));
        assert_eq!(parse_dimension("auto").unwrap(), Dimension::Auto);
        assert!(parse_dimension("50p").is_err());
    }

    #[test]
    fn test_parse_shorthand_margins() {
        let m1 = parse_shorthand_margins("10pt").unwrap();
        assert_eq!(
            m1,
            Margins {
                top: 10.0,
                right: 10.0,
                bottom: 10.0,
                left: 10.0
            }
        );
        let m2 = parse_shorthand_margins("10pt 20pt").unwrap();
        assert_eq!(
            m2,
            Margins {
                top: 10.0,
                right: 20.0,
                bottom: 10.0,
                left: 20.0
            }
        );
        let m4 = parse_shorthand_margins("10 20 30 40").unwrap();
        assert_eq!(
            m4,
            Margins {
                top: 10.0,
                right: 20.0,
                bottom: 30.0,
                left: 40.0
            }
        );
        assert!(parse_shorthand_margins("10 20 30").is_err());
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(
            parse_color("#FF0000").unwrap(),
            Color {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0
            }
        );
        assert_eq!(
            parse_color("#f00").unwrap(),
            Color {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0
            }
        );
        assert_eq!(
            parse_color("#3a3").unwrap(),
            Color {
                r: 51,
                g: 170,
                b: 51,
                a: 1.0
            }
        );
        assert!(parse_color("red").is_err());
        assert!(parse_color("#1234").is_err());
    }

    #[test]
    fn test_parse_border() {
        let border = parse_border("2pt solid #00ff00").unwrap();
        assert_eq!(border.width, 2.0);
        assert_eq!(border.style, BorderStyle::Solid);
        assert_eq!(
            border.color,
            Color {
                r: 0,
                g: 255,
                b: 0,
                a: 1.0
            }
        );
        assert!(parse_border("2pt solid").is_err());
    }

    #[test]
    fn test_parse_font_properties() {
        assert_eq!(parse_font_weight("bold").unwrap(), FontWeight::Bold);
        assert_eq!(parse_font_weight("700").unwrap(), FontWeight::Numeric(700));
        assert_eq!(parse_font_style("italic").unwrap(), FontStyle::Italic);
        assert_eq!(parse_text_align("center").unwrap(), TextAlign::Center);
    }

    #[test]
    fn test_deserialize_font_weight() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Test {
            weight: FontWeight,
        }

        let from_str: Test = serde_json::from_str(r#"{ "weight": "bold" }"#).unwrap();
        assert_eq!(from_str.weight, FontWeight::Bold);

        let from_num_str: Test = serde_json::from_str(r#"{ "weight": "700" }"#).unwrap();
        assert_eq!(from_num_str.weight, FontWeight::Numeric(700));

        let from_num: Test = serde_json::from_str(r#"{ "weight": 700 }"#).unwrap();
        assert_eq!(from_num.weight, FontWeight::Numeric(700));

        let from_str_normal: Test = serde_json::from_str(r#"{ "weight": "normal" }"#).unwrap();
        assert_eq!(from_str_normal.weight, FontWeight::Regular);

        assert!(serde_json::from_str::<Test>(r#"{ "weight": "heavy" }"#).is_err());
    }

    #[test]
    fn test_parse_page_size() {
        assert_eq!(parse_page_size("a4").unwrap(), PageSize::A4);
        assert_eq!(parse_page_size("Letter").unwrap(), PageSize::Letter);
        assert!(parse_page_size("A3").is_err());
    }

    #[test]
    fn test_parse_inline_css() {
        let mut style = ElementStyle::default();
        parse_inline_css("font-size: 16pt; color: #f00; margin-left: 5mm", &mut style).unwrap();
        assert_eq!(style.font_size, Some(16.0));
        assert_eq!(
            style.color,
            Some(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0
            })
        );
        assert!((style.margin.unwrap().left - (5.0 * 2.835)).abs() < 0.001);
    }

    #[test]
    fn test_parse_fo_attributes() {
        let mut style = ElementStyle::default();
        let attrs = vec![
            (b"font-size".to_vec(), b"12pt".to_vec()),
            (b"padding".to_vec(), b"5pt 10pt".to_vec()),
        ];
        parse_fo_attributes(&attrs, &mut style).unwrap();
        assert_eq!(style.font_size, Some(12.0));
        let padding = style.padding.unwrap();
        assert_eq!(padding.top, 5.0);
        assert_eq!(padding.right, 10.0);
        assert_eq!(padding.bottom, 5.0);
        assert_eq!(padding.left, 10.0);
    }

    #[test]
    fn test_deserialize_color() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Test {
            c: Color,
        }

        let from_str: Test = serde_json::from_str("{ \"c\": \"#ff8800\" }").unwrap();
        assert_eq!(
            from_str,
            Test {
                c: Color {
                    r: 255,
                    g: 136,
                    b: 0,
                    a: 1.0
                }
            }
        );

        let from_map: Test =
            serde_json::from_str("{ \"c\": { \"r\": 255, \"g\": 136, \"b\": 0 } }").unwrap();
        assert_eq!(
            from_map,
            Test {
                c: Color {
                    r: 255,
                    g: 136,
                    b: 0,
                    a: 1.0
                }
            }
        );

        let from_map_alpha: Test =
            serde_json::from_str("{ \"c\": { \"r\": 255, \"g\": 136, \"b\": 0, \"a\": 0.5 } }")
                .unwrap();
        assert_eq!(
            from_map_alpha,
            Test {
                c: Color {
                    r: 255,
                    g: 136,
                    b: 0,
                    a: 0.5
                }
            }
        );
    }

    #[test]
    fn test_deserialize_border() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Test {
            b: Border,
        }

        let from_str: Test = serde_json::from_str("{ \"b\": \"1.5pt dashed #123\" }").unwrap();
        assert_eq!(from_str.b.width, 1.5);
        assert_eq!(from_str.b.style, BorderStyle::Dashed);
        assert_eq!(
            from_str.b.color,
            Color {
                r: 17,
                g: 34,
                b: 51,
                a: 1.0
            }
        );

        let from_map_str = "{ \"b\": { \"width\": 2.0, \"style\": \"Solid\", \"color\": \"#ff0000\" } }";
        let from_map: Test = serde_json::from_str(from_map_str).unwrap();
        assert_eq!(from_map.b.width, 2.0);
        assert_eq!(from_map.b.style, BorderStyle::Solid);
        assert_eq!(
            from_map.b.color,
            Color {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0
            }
        );
    }

    #[test]
    fn test_deserialize_page_size() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Test {
            size: PageSize,
        }

        // Test case-insensitive string variants
        let from_str_uc: Test = serde_json::from_str(r#"{ "size": "Letter" }"#).unwrap();
        assert_eq!(from_str_uc.size, PageSize::Letter);

        let from_str_lc: Test = serde_json::from_str(r#"{ "size": "letter" }"#).unwrap();
        assert_eq!(from_str_lc.size, PageSize::Letter);

        let from_str_a4: Test = serde_json::from_str(r#"{ "size": "a4" }"#).unwrap();
        assert_eq!(from_str_a4.size, PageSize::A4);

        // Test custom size from map
        let from_map: Test =
            serde_json::from_str(r#"{ "size": { "width": 600.0, "height": 800.0 } }"#).unwrap();
        assert_eq!(
            from_map.size,
            PageSize::Custom {
                width: 600.0,
                height: 800.0
            }
        );

        // Test invalid string
        assert!(serde_json::from_str::<Test>(r#"{ "size": "Tabloid" }"#).is_err());
    }
}