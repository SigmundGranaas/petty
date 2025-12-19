//! A collection of small, composable, and thoroughly tested parser functions
//! for all CSS-like style values, built using the `nom` parser-combinator library.

use crate::core::style::border::{Border, BorderStyle};
use crate::core::style::dimension::{Dimension, Margins};
use crate::parser::ParseError;
use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_while_m_n};
use nom::character::complete::{char, space0, space1};
use nom::combinator::{map, map_res, opt, recognize};
use nom::multi::separated_list1;
use nom::sequence::{delimited, pair, preceded, tuple};
use nom::IResult;
use crate::core::base::color::Color;
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

pub fn parse_length(input: &str) -> IResult<&str, f32> {
    let (input, value) = parse_f32(input)?;
    let (input, unit_multiplier) = opt(parse_unit)(input)?;
    Ok((input, value * unit_multiplier.unwrap_or(1.0)))
}

pub fn parse_dimension(input: &str) -> IResult<&str, Dimension> {
    alt((
        map(tag("auto"), |_| Dimension::Auto),
        map(
            pair(parse_f32, char('%')),
            |(val, _)| Dimension::Percent(val),
        ),
        map(parse_length, Dimension::Pt),
    ))(input)
}

pub fn parse_shorthand_margins(input: &str) -> Result<Margins, ParseError> {
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
            _ => Err(ParseError::Nom(format!(
                "Invalid number of values for margin/padding shorthand: got {}, expected 1, 2, or 4.",
                parts.len()
            ))),
        },
        _ => Err(ParseError::Nom(format!("Failed to parse margins value: '{}'", input))),
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

pub fn parse_color(input: &str) -> IResult<&str, Color> {
    preceded(char('#'), alt((hex_color_6, hex_color_3)))(input)
}

pub fn parse_border_style(input: &str) -> IResult<&str, BorderStyle> {
    alt((
        map(tag_no_case("solid"), |_| BorderStyle::Solid),
        map(tag_no_case("dashed"), |_| BorderStyle::Dashed),
        map(tag_no_case("dotted"), |_| BorderStyle::Dotted),
        map(tag_no_case("double"), |_| BorderStyle::Double),
        map(tag_no_case("none"), |_| BorderStyle::None),
    ))(input)
}

pub fn parse_border(input: &str) -> IResult<&str, Border> {
    map(
        tuple((ws(parse_length), ws(parse_border_style), ws(parse_color))),
        |(width, style, color)| Border { width, style, color },
    )(input)
}

/// Helper to run a nom parser and convert its result to a `Result<T, ParseError>`.
pub fn run_parser<'a, T, F>(parser: F, input: &'a str) -> Result<T, ParseError>
where
    F: Fn(&'a str) -> IResult<&'a str, T>,
{
    match parser(input.trim()) {
        Ok(("", result)) => Ok(result),
        Ok((rem, _)) => Err(ParseError::Nom(format!(
            "Parser did not consume all input. Remainder: '{}'",
            rem
        ))),
        Err(e) => Err(ParseError::Nom(e.to_string())),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_length_nom() {
        assert_eq!(run_parser(parse_length, "12pt").unwrap(), 12.0);
        assert_eq!(run_parser(parse_length, " 1in ").unwrap(), 72.0);
        assert_eq!(run_parser(parse_length, "10mm").unwrap(), 28.35);
        assert_eq!(run_parser(parse_length, "10").unwrap(), 10.0);
        assert!(run_parser(parse_length, "abc").is_err());
    }

    #[test]
    fn test_parse_dimension_nom() {
        assert_eq!(run_parser(parse_dimension, "12pt").unwrap(), Dimension::Pt(12.0));
        assert_eq!(run_parser(parse_dimension, "50%").unwrap(), Dimension::Percent(50.0));
        assert_eq!(run_parser(parse_dimension, "auto").unwrap(), Dimension::Auto);
        assert!(run_parser(parse_dimension, "50p").is_err());
    }

    #[test]
    fn test_parse_shorthand_margins_nom() {
        let m1 = parse_shorthand_margins("10pt").unwrap();
        assert_eq!(m1, Margins { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 });

        let m2 = parse_shorthand_margins("10pt 20pt").unwrap();
        assert_eq!(m2, Margins { top: 10.0, right: 20.0, bottom: 10.0, left: 20.0 });

        let m4 = parse_shorthand_margins("10 20 30 40").unwrap();
        assert_eq!(m4, Margins { top: 10.0, right: 20.0, bottom: 30.0, left: 40.0 });

        assert!(parse_shorthand_margins("10 20 30").is_err());
    }

    #[test]
    fn test_parse_color_nom() {
        assert_eq!(run_parser(parse_color, "#FF0000").unwrap(), Color { r: 255, g: 0, b: 0, a: 1.0 });
        assert_eq!(run_parser(parse_color, "#f00").unwrap(), Color { r: 255, g: 0, b: 0, a: 1.0 });
        assert_eq!(run_parser(parse_color, "#3a3").unwrap(), Color { r: 51, g: 170, b: 51, a: 1.0 });
        assert!(run_parser(parse_color, "red").is_err());
        assert!(run_parser(parse_color, "#1234").is_err());
    }

    #[test]
    fn test_parse_border_nom() {
        let border = run_parser(parse_border, "2pt solid #00ff00").unwrap();
        assert_eq!(border.width, 2.0);
        assert_eq!(border.style, BorderStyle::Solid);
        assert_eq!(border.color, Color { r: 0, g: 255, b: 0, a: 1.0 });

        let border_spaced = run_parser(parse_border, "  2.5pt   dashed   #112233  ").unwrap();
        assert_eq!(border_spaced.width, 2.5);
        assert_eq!(border_spaced.style, BorderStyle::Dashed);
        assert_eq!(border_spaced.color, Color { r: 17, g: 34, b: 51, a: 1.0 });

        assert!(run_parser(parse_border, "2pt solid").is_err());
    }
}