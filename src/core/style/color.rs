// src/core/color.rs
use serde::{de, Deserialize, Deserializer, Serialize};

fn default_alpha() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: f32,
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColorVisitor;

        impl<'de> de::Visitor<'de> for ColorVisitor {
            type Value = Color;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a hex color string like '#RRGGBB' or a map like { \"r\": 255, ... }",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Color, E>
            where
                E: de::Error,
            {
                crate::parser::style::parse_color(value).map_err(E::custom)
            }

            fn visit_map<M>(self, map: M) -> Result<Color, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct ColorMap {
                    r: u8,
                    g: u8,
                    b: u8,
                    #[serde(default = "default_alpha")]
                    a: f32,
                }
                let color_map =
                    ColorMap::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(Color {
                    r: color_map.r,
                    g: color_map.g,
                    b: color_map.b,
                    a: color_map.a,
                })
            }
        }

        deserializer.deserialize_any(ColorVisitor)
    }
}