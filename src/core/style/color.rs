use crate::parser::style_parsers;
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: f32,
}

fn default_alpha() -> f32 {
    1.0
}

impl Default for Color {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0, a: 1.0 }
    }
}


impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ColorDef {
            Str(String),
            Map { r: u8, g: u8, b: u8, a: Option<f32> },
        }

        match ColorDef::deserialize(deserializer)? {
            ColorDef::Str(s) => {
                // REPLACED: Call the new, robust nom parser.
                style_parsers::run_parser(style_parsers::parse_color, &s).map_err(de::Error::custom)
            }
            ColorDef::Map { r, g, b, a } => Ok(Color { r, g, b, a: a.unwrap_or(1.0) }),
        }
    }
}