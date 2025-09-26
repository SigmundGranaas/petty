// src/core/border.rs
use serde::{de, Deserialize, Deserializer, Serialize};
use crate::core::style::color::Color;

#[derive(Debug, Clone, Serialize, PartialEq)]
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
        struct BorderVisitor;

        impl<'de> de::Visitor<'de> for BorderVisitor {
            type Value = Border;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string like '1pt solid #000' or a map")
            }

            fn visit_str<E>(self, value: &str) -> Result<Border, E>
            where
                E: de::Error,
            {
                crate::parser::style::parse_border(value).map_err(E::custom)
            }

            fn visit_map<M>(self, map: M) -> Result<Border, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct BorderMap {
                    width: f32,
                    style: BorderStyle,
                    color: Color,
                }
                let border_map =
                    BorderMap::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(Border {
                    width: border_map.width,
                    style: border_map.style,
                    color: border_map.color,
                })
            }
        }

        deserializer.deserialize_any(BorderVisitor)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum BorderStyle {
    Solid,
    Dashed,
    Dotted,
    Double,
    None,
}

impl<'de> Deserialize<'de> for BorderStyle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "solid" => Ok(BorderStyle::Solid),
            "dashed" => Ok(BorderStyle::Dashed),
            "dotted" => Ok(BorderStyle::Dotted),
            "double" => Ok(BorderStyle::Double),
            "none" => Ok(BorderStyle::None),
            _ => Err(de::Error::custom(format!("unknown border style: {}", s))),
        }
    }
}