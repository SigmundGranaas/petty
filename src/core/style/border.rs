use super::color::Color;
use crate::parser::ParseError;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

impl FromStr for BorderStyle {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(BorderStyle::None),
            "solid" => Ok(BorderStyle::Solid),
            "dashed" => Ok(BorderStyle::Dashed),
            "dotted" => Ok(BorderStyle::Dotted),
            "double" => Ok(BorderStyle::Double),
            _ => Err(ParseError::TemplateParse(format!("Invalid border style: '{}'", s))),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Border {
    pub width: f32,
    pub style: BorderStyle,
    pub color: Color,
}

impl From<(f32, &str, Color)> for Border {
    fn from((width, style_str, color): (f32, &str, Color)) -> Self {
        let style = BorderStyle::from_str(style_str).unwrap_or(BorderStyle::Solid);
        Self { width, style, color }
    }
}