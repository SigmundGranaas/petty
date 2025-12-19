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

    /// Parse a hex color string (#RGB or #RRGGBB format)
    fn parse_hex(s: &str) -> Result<Color, String> {
        let s = s.trim();
        if !s.starts_with('#') {
            return Err(format!("Color must start with #, got: {}", s));
        }
        let hex = &s[1..];

        match hex.len() {
            3 => {
                // #RGB format - expand each digit
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)
                    .map_err(|e| format!("Invalid red component: {}", e))?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)
                    .map_err(|e| format!("Invalid green component: {}", e))?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)
                    .map_err(|e| format!("Invalid blue component: {}", e))?;
                Ok(Color { r, g, b, a: 1.0 })
            }
            6 => {
                // #RRGGBB format
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|e| format!("Invalid red component: {}", e))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|e| format!("Invalid green component: {}", e))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|e| format!("Invalid blue component: {}", e))?;
                Ok(Color { r, g, b, a: 1.0 })
            }
            _ => Err(format!("Invalid hex color length: expected 3 or 6, got {}", hex.len()))
        }
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
                Self::parse_hex(&s).map_err(de::Error::custom)
            }
            ColorDef::Map { r, g, b, a } => Ok(Color { r, g, b, a }),
        }
    }
}
