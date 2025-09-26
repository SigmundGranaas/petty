// src/core/dimension.rs
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Dimension {
    Px(f32),
    Pt(f32),
    Percent(f32),
    Auto,
}

impl<'de> Deserialize<'de> for Dimension {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        crate::parser::style::parse_dimension(&s)
            .map_err(|e| de::Error::custom(format!("invalid dimension string: '{}' ({})", s, e)))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Margins {
    #[serde(default)]
    pub top: f32,
    #[serde(default)]
    pub right: f32,
    #[serde(default)]
    pub bottom: f32,
    #[serde(default)]
    pub left: f32,
}

impl<'de> Deserialize<'de> for Margins {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MarginsVisitor;

        impl<'de> de::Visitor<'de> for MarginsVisitor {
            type Value = Margins;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a string like '10pt' or '10pt 20pt', or a map like { \"top\": 10.0 }",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Margins, E>
            where
                E: de::Error,
            {
                crate::parser::style::parse_shorthand_margins(value).map_err(E::custom)
            }

            fn visit_map<M>(self, map: M) -> Result<Margins, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                #[derive(Deserialize, Default)]
                struct MarginMap {
                    #[serde(default)]
                    top: f32,
                    #[serde(default)]
                    right: f32,
                    #[serde(default)]
                    bottom: f32,
                    #[serde(default)]
                    left: f32,
                }
                let margin_map =
                    MarginMap::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(Margins {
                    top: margin_map.top,
                    right: margin_map.right,
                    bottom: margin_map.bottom,
                    left: margin_map.left,
                })
            }
        }

        deserializer.deserialize_any(MarginsVisitor)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum PageSize {
    A4,
    Letter,
    Legal,
    Custom { width: f32, height: f32 },
}

impl<'de> Deserialize<'de> for PageSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PageSizeVisitor;

        impl<'de> de::Visitor<'de> for PageSizeVisitor {
            type Value = PageSize;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a page size string like 'A4' or 'Letter', or a map like { \"width\": 595, \"height\": 842 }",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<PageSize, E>
            where
                E: de::Error,
            {
                match value.to_lowercase().as_str() {
                    "a4" => Ok(PageSize::A4),
                    "letter" => Ok(PageSize::Letter),
                    "legal" => Ok(PageSize::Legal),
                    _ => Err(E::custom(format!("unknown page size: '{}'", value))),
                }
            }

            fn visit_map<M>(self, map: M) -> Result<PageSize, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct CustomSize {
                    width: f32,
                    height: f32,
                }

                let custom = CustomSize::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(PageSize::Custom {
                    width: custom.width,
                    height: custom.height,
                })
            }
        }

        deserializer.deserialize_any(PageSizeVisitor)
    }
}


impl Default for PageSize {
    fn default() -> Self {
        PageSize::A4
    }
}