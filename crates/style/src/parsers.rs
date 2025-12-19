//! Low-level nom parser functions for CSS-like style values.
//!
//! This module provides composable parser functions for parsing style values
//! like lengths, dimensions, colors, and borders.

use crate::border::{Border, BorderStyle};
use crate::dimension::{Dimension, Margins, PageSize};
use crate::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use crate::font::{FontStyle, FontWeight};
use crate::list::ListStyleType;
use crate::text::TextAlign;
use nom::IResult;
use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_while_m_n};
use nom::character::complete::{char, space0, space1};
use nom::combinator::{map, map_res, opt, recognize};
use nom::multi::separated_list1;
use nom::sequence::{delimited, pair, preceded, tuple};
use petty_types::Color;
use thiserror::Error;

/// Errors that can occur during style parsing.
#[derive(Error, Debug, Clone)]
pub enum StyleParseError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid value for '{property}': {value}")]
    InvalidValue { property: String, value: String },

    #[error("Float parse error: {0}")]
    FloatParse(String),
}

// --- Helper Parsers ---

fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(space0, inner, space0)
}

fn parse_f32(input: &str) -> IResult<&str, f32> {
    map_res(
        recognize(pair(
            opt(alt((char('+'), char('-')))),
            alt((
                recognize(tuple((
                    take_while_m_n(1, 10, |c: char| c.is_ascii_digit()),
                    opt(tuple((
                        char('.'),
                        take_while_m_n(1, 10, |c: char| c.is_ascii_digit()),
                    ))),
                ))),
                recognize(tuple((
                    char('.'),
                    take_while_m_n(1, 10, |c: char| c.is_ascii_digit()),
                ))),
            )),
        )),
        |s: &str| s.parse::<f32>(),
    )(input)
}

// --- Unit & Dimension Parsers ---

fn parse_unit(input: &str) -> IResult<&str, f32> {
    alt((
        map(tag_no_case("pt"), |_| 1.0),
        map(tag_no_case("px"), |_| 1.0), // Treat px as pt
        map(tag_no_case("in"), |_| 72.0),
        map(tag_no_case("cm"), |_| 28.35),
        map(tag_no_case("mm"), |_| 2.835),
    ))(input)
}

/// Parses a length value with optional unit (e.g., "12pt", "1in", "10mm").
pub fn parse_length(input: &str) -> IResult<&str, f32> {
    let (input, value) = parse_f32(input)?;
    let (input, unit_multiplier) = opt(parse_unit)(input)?;
    Ok((input, value * unit_multiplier.unwrap_or(1.0)))
}

/// Parses a dimension value (length, percentage, or "auto").
pub fn parse_dimension(input: &str) -> IResult<&str, Dimension> {
    alt((
        map(tag("auto"), |_| Dimension::Auto),
        map(pair(parse_f32, char('%')), |(val, _)| {
            Dimension::Percent(val)
        }),
        map(parse_length, Dimension::Pt),
    ))(input)
}

/// Parses CSS shorthand margins (1, 2, or 4 values).
pub fn parse_shorthand_margins(input: &str) -> Result<Margins, StyleParseError> {
    let parts_res = separated_list1(space1, parse_length)(input.trim());

    match parts_res {
        Ok(("", parts)) => match parts.len() {
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
            _ => Err(StyleParseError::Parse(format!(
                "Invalid number of values for margin/padding shorthand: got {}, expected 1, 2, or 4.",
                parts.len()
            ))),
        },
        _ => Err(StyleParseError::Parse(format!(
            "Failed to parse margins value: '{}'",
            input
        ))),
    }
}

// --- Color & Border Parsers ---

fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str_radix(input, 16)
}

fn is_hex_digit(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn hex_primary(input: &str) -> IResult<&str, u8> {
    map_res(take_while_m_n(2, 2, is_hex_digit), from_hex)(input)
}

fn hex_color_6(input: &str) -> IResult<&str, Color> {
    map(
        tuple((hex_primary, hex_primary, hex_primary)),
        |(r, g, b)| Color { r, g, b, a: 1.0 },
    )(input)
}

fn hex_color_3(input: &str) -> IResult<&str, Color> {
    map(
        tuple((
            take_while_m_n(1, 1, is_hex_digit),
            take_while_m_n(1, 1, is_hex_digit),
            take_while_m_n(1, 1, is_hex_digit),
        )),
        |(r_s, g_s, b_s): (&str, &str, &str)| Color {
            r: from_hex(&format!("{}{}", r_s, r_s)).unwrap(),
            g: from_hex(&format!("{}{}", g_s, g_s)).unwrap(),
            b: from_hex(&format!("{}{}", b_s, b_s)).unwrap(),
            a: 1.0,
        },
    )(input)
}

/// Parses a hex color (e.g., "#FF0000" or "#F00").
pub fn parse_color(input: &str) -> IResult<&str, Color> {
    preceded(char('#'), alt((hex_color_6, hex_color_3)))(input)
}

/// Parses a border style keyword.
pub fn parse_border_style(input: &str) -> IResult<&str, BorderStyle> {
    alt((
        map(tag_no_case("solid"), |_| BorderStyle::Solid),
        map(tag_no_case("dashed"), |_| BorderStyle::Dashed),
        map(tag_no_case("dotted"), |_| BorderStyle::Dotted),
        map(tag_no_case("double"), |_| BorderStyle::Double),
        map(tag_no_case("none"), |_| BorderStyle::None),
    ))(input)
}

/// Parses a CSS border shorthand (e.g., "2pt solid #00ff00").
pub fn parse_border(input: &str) -> IResult<&str, Border> {
    map(
        tuple((ws(parse_length), ws(parse_border_style), ws(parse_color))),
        |(width, style, color)| Border {
            width,
            style,
            color,
        },
    )(input)
}

/// Helper to run a nom parser and convert its result to a `Result<T, StyleParseError>`.
pub fn run_parser<'a, T, F>(parser: F, input: &'a str) -> Result<T, StyleParseError>
where
    F: Fn(&'a str) -> IResult<&'a str, T>,
{
    match parser(input.trim()) {
        Ok(("", result)) => Ok(result),
        Ok((rem, _)) => Err(StyleParseError::Parse(format!(
            "Parser did not consume all input. Remainder: '{}'",
            rem
        ))),
        Err(e) => Err(StyleParseError::Parse(e.to_string())),
    }
}

// --- High-level Parse Functions ---

/// Parses a font weight string (e.g., "bold", "400").
pub fn parse_font_weight(s: &str) -> Result<FontWeight, StyleParseError> {
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
                .map_err(|_| StyleParseError::InvalidValue {
                    property: "font-weight".to_string(),
                    value: s.to_string(),
                })?;
            Ok(FontWeight::Numeric(num_weight))
        }
    }
}

/// Parses a font style string (e.g., "normal", "italic").
pub fn parse_font_style(s: &str) -> Result<FontStyle, StyleParseError> {
    match s.to_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(StyleParseError::InvalidValue {
            property: "font-style".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses a text-align value.
pub fn parse_text_align(s: &str) -> Result<TextAlign, StyleParseError> {
    match s.to_lowercase().as_str() {
        "left" => Ok(TextAlign::Left),
        "right" => Ok(TextAlign::Right),
        "center" => Ok(TextAlign::Center),
        "justify" => Ok(TextAlign::Justify),
        _ => Err(StyleParseError::InvalidValue {
            property: "text-align".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses a list-style-type value.
pub fn parse_list_style_type(s: &str) -> Result<ListStyleType, StyleParseError> {
    match s.to_lowercase().as_str() {
        "disc" => Ok(ListStyleType::Disc),
        "circle" => Ok(ListStyleType::Circle),
        "square" => Ok(ListStyleType::Square),
        "decimal" => Ok(ListStyleType::Decimal),
        "none" => Ok(ListStyleType::None),
        _ => Err(StyleParseError::InvalidValue {
            property: "list-style-type".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses a flex-direction value.
pub fn parse_flex_direction(s: &str) -> Result<FlexDirection, StyleParseError> {
    match s.to_lowercase().as_str() {
        "row" => Ok(FlexDirection::Row),
        "row-reverse" => Ok(FlexDirection::RowReverse),
        "column" => Ok(FlexDirection::Column),
        "column-reverse" => Ok(FlexDirection::ColumnReverse),
        _ => Err(StyleParseError::InvalidValue {
            property: "flex-direction".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses a flex-wrap value.
pub fn parse_flex_wrap(s: &str) -> Result<FlexWrap, StyleParseError> {
    match s.to_lowercase().as_str() {
        "nowrap" => Ok(FlexWrap::NoWrap),
        "wrap" => Ok(FlexWrap::Wrap),
        "wrap-reverse" => Ok(FlexWrap::WrapReverse),
        _ => Err(StyleParseError::InvalidValue {
            property: "flex-wrap".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses a justify-content value.
pub fn parse_justify_content(s: &str) -> Result<JustifyContent, StyleParseError> {
    match s.to_lowercase().as_str() {
        "flex-start" => Ok(JustifyContent::FlexStart),
        "flex-end" => Ok(JustifyContent::FlexEnd),
        "center" => Ok(JustifyContent::Center),
        "space-between" => Ok(JustifyContent::SpaceBetween),
        "space-around" => Ok(JustifyContent::SpaceAround),
        "space-evenly" => Ok(JustifyContent::SpaceEvenly),
        _ => Err(StyleParseError::InvalidValue {
            property: "justify-content".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses an align-items value.
pub fn parse_align_items(s: &str) -> Result<AlignItems, StyleParseError> {
    match s.to_lowercase().as_str() {
        "stretch" => Ok(AlignItems::Stretch),
        "flex-start" => Ok(AlignItems::FlexStart),
        "flex-end" => Ok(AlignItems::FlexEnd),
        "center" => Ok(AlignItems::Center),
        "baseline" => Ok(AlignItems::Baseline),
        _ => Err(StyleParseError::InvalidValue {
            property: "align-items".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses an align-self value.
pub fn parse_align_self(s: &str) -> Result<AlignSelf, StyleParseError> {
    match s.to_lowercase().as_str() {
        "auto" => Ok(AlignSelf::Auto),
        "stretch" => Ok(AlignSelf::Stretch),
        "flex-start" => Ok(AlignSelf::FlexStart),
        "flex-end" => Ok(AlignSelf::FlexEnd),
        "center" => Ok(AlignSelf::Center),
        "baseline" => Ok(AlignSelf::Baseline),
        _ => Err(StyleParseError::InvalidValue {
            property: "align-self".to_string(),
            value: s.to_string(),
        }),
    }
}

/// Parses a page size value.
pub fn parse_page_size(s: &str) -> Result<PageSize, StyleParseError> {
    match s.to_lowercase().as_str() {
        "a4" => Ok(PageSize::A4),
        "letter" => Ok(PageSize::Letter),
        "legal" => Ok(PageSize::Legal),
        _ => Err(StyleParseError::InvalidValue {
            property: "page-size".to_string(),
            value: s.to_string(),
        }),
    }
}

// --- High-level Style Application Functions ---

use crate::stylesheet::ElementStyle;

/// Applies a single parsed style property to an `ElementStyle` struct.
/// This is the central dispatcher for applying individual CSS-like properties.
pub fn apply_style_property(
    style: &mut ElementStyle,
    attr_name: &str,
    value: &str,
) -> Result<(), StyleParseError> {
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
        "margin-top" => {
            style.margin.get_or_insert_with(Default::default).top = run_parser(parse_length, value)?
        }
        "margin-right" => {
            style.margin.get_or_insert_with(Default::default).right =
                run_parser(parse_length, value)?
        }
        "margin-bottom" => {
            style.margin.get_or_insert_with(Default::default).bottom =
                run_parser(parse_length, value)?
        }
        "margin-left" => {
            style.margin.get_or_insert_with(Default::default).left =
                run_parser(parse_length, value)?
        }
        "padding" => style.padding = Some(parse_shorthand_margins(value)?),
        "padding-top" => {
            style.padding.get_or_insert_with(Default::default).top =
                run_parser(parse_length, value)?
        }
        "padding-right" => {
            style.padding.get_or_insert_with(Default::default).right =
                run_parser(parse_length, value)?
        }
        "padding-bottom" => {
            style.padding.get_or_insert_with(Default::default).bottom =
                run_parser(parse_length, value)?
        }
        "padding-left" => {
            style.padding.get_or_insert_with(Default::default).left =
                run_parser(parse_length, value)?
        }
        "width" => style.width = Some(run_parser(parse_dimension, value)?),
        "height" => style.height = Some(run_parser(parse_dimension, value)?),
        "list-style-type" => style.list_style_type = Some(parse_list_style_type(value)?),
        "flex-direction" => style.flex_direction = Some(parse_flex_direction(value)?),
        "flex-wrap" => style.flex_wrap = Some(parse_flex_wrap(value)?),
        "justify-content" => style.justify_content = Some(parse_justify_content(value)?),
        "align-items" => style.align_items = Some(parse_align_items(value)?),
        "flex-grow" => {
            style.flex_grow = Some(value.trim().parse::<f32>().map_err(|_| {
                StyleParseError::FloatParse(format!("Invalid flex-grow value: {}", value))
            })?)
        }
        "flex-shrink" => {
            style.flex_shrink = Some(value.trim().parse::<f32>().map_err(|_| {
                StyleParseError::FloatParse(format!("Invalid flex-shrink value: {}", value))
            })?)
        }
        "flex-basis" => style.flex_basis = Some(run_parser(parse_dimension, value)?),
        "align-self" => style.align_self = Some(parse_align_self(value)?),
        _ => {} // Not a style attribute, ignore.
    };
    Ok(())
}

/// Parses XSL-FO attributes on an XML tag into an `ElementStyle` object.
pub fn parse_fo_attributes(
    attrs: &[(Vec<u8>, Vec<u8>)],
    style_override: &mut ElementStyle,
) -> Result<(), StyleParseError> {
    for (key, value) in attrs {
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
) -> Result<(), StyleParseError> {
    for declaration in css.split(';') {
        if let Some((key, value)) = declaration.split_once(':') {
            apply_style_property(style_override, key.trim(), value.trim())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_length() {
        assert_eq!(run_parser(parse_length, "12pt").unwrap(), 12.0);
        assert_eq!(run_parser(parse_length, " 1in ").unwrap(), 72.0);
        assert_eq!(run_parser(parse_length, "10mm").unwrap(), 28.35);
        assert_eq!(run_parser(parse_length, "10").unwrap(), 10.0);
        assert!(run_parser(parse_length, "abc").is_err());
    }

    #[test]
    fn test_parse_dimension() {
        assert_eq!(
            run_parser(parse_dimension, "12pt").unwrap(),
            Dimension::Pt(12.0)
        );
        assert_eq!(
            run_parser(parse_dimension, "50%").unwrap(),
            Dimension::Percent(50.0)
        );
        assert_eq!(
            run_parser(parse_dimension, "auto").unwrap(),
            Dimension::Auto
        );
        assert!(run_parser(parse_dimension, "50p").is_err());
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
            run_parser(parse_color, "#FF0000").unwrap(),
            Color {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0
            }
        );
        assert_eq!(
            run_parser(parse_color, "#f00").unwrap(),
            Color {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0
            }
        );
        assert!(run_parser(parse_color, "red").is_err());
    }

    #[test]
    fn test_parse_border() {
        let border = run_parser(parse_border, "2pt solid #00ff00").unwrap();
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
    }

    #[test]
    fn test_parse_font_weight() {
        assert_eq!(parse_font_weight("bold").unwrap(), FontWeight::Bold);
        assert_eq!(parse_font_weight("400").unwrap(), FontWeight::Numeric(400));
        assert!(parse_font_weight("invalid").is_err());
    }

    #[test]
    fn test_parse_page_size() {
        assert_eq!(parse_page_size("A4").unwrap(), PageSize::A4);
        assert_eq!(parse_page_size("letter").unwrap(), PageSize::Letter);
        assert!(parse_page_size("unknown").is_err());
    }
}
