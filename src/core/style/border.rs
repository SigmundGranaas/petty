use super::color::Color;
use crate::parser::style_parsers;
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Border {
    pub width: f32,
    pub style: BorderStyle,
    pub color: Color,
}

impl<'de> Deserialize<'de> for Border {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum BorderDef {
            Str(String),
            Map { width: f32, style: BorderStyle, color: Color },
        }

        match BorderDef::deserialize(deserializer)? {
            BorderDef::Str(s) => {
                // REPLACED: Call the new, robust nom parser.
                style_parsers::run_parser(style_parsers::parse_border, &s).map_err(de::Error::custom)
            }
            BorderDef::Map { width, style, color } => Ok(Border { width, style, color }),
        }
    }
}