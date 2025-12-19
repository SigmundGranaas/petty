use crate::parser::style_parsers;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::hash::{Hash, Hasher};

fn default_one() -> f32 {
    1.0
}

fn is_one(num: &f32) -> bool {
    *num == 1.0
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(skip_serializing_if = "is_one", default = "default_one")]
    pub a: f32,
}

impl Eq for Color {}

impl Hash for Color {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.r.hash(state);
        self.g.hash(state);
        self.b.hash(state);
        self.a.to_bits().hash(state);
    }
}

impl Default for Color {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0, a: 1.0 }
    }
}

impl Color {
    pub fn gray(value: u8) -> Self {
        Self { r: value, g: value, b: value, a: 1.0 }
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
            Map { r: u8, g: u8, b: u8, #[serde(default = "default_one")] a: f32 },
        }

        match ColorDef::deserialize(deserializer)? {
            ColorDef::Str(s) => {
                style_parsers::run_parser(style_parsers::parse_color, &s).map_err(de::Error::custom)
            }
            ColorDef::Map { r, g, b, a } => Ok(Color { r, g, b, a }),
        }
    }
}