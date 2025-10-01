//! Defines primitives for size, position, and spacing.
use crate::parser::style_parsers;
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Dimension {
    Pt(f32),
    Percent(f32),
    Auto,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Margins {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
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
                formatter.write_str("a string like '10pt' or '10pt 20pt' or a map")
            }

            fn visit_str<E>(self, value: &str) -> Result<Margins, E>
            where
                E: de::Error,
            {
                style_parsers::parse_shorthand_margins(value).map_err(E::custom)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Margins, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut margins = Margins::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "top" => margins.top = map.next_value()?,
                        "right" => margins.right = map.next_value()?,
                        "bottom" => margins.bottom = map.next_value()?,
                        "left" => margins.left = map.next_value()?,
                        _ => { /* ignore unknown fields */ }
                    }
                }
                Ok(margins)
            }
        }
        deserializer.deserialize_any(MarginsVisitor)
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub enum PageSize {
    A4,
    Letter,
    Legal,
    Custom { width: f32, height: f32 },
}

impl PageSize {
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            PageSize::A4 => (595.28, 841.89),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Custom { width, height } => (*width, *height),
        }
    }

    pub fn set_width(&mut self, new_width: f32) {
        match self {
            PageSize::Custom { width, .. } => *width = new_width,
            _ => *self = PageSize::Custom {
                width: new_width,
                height: self.dimensions_pt().1,
            },
        }
    }

    pub fn set_height(&mut self, new_height: f32) {
        match self {
            PageSize::Custom { height, .. } => *height = new_height,
            _ => *self = PageSize::Custom {
                width: self.dimensions_pt().0,
                height: new_height,
            },
        }
    }
}

impl Default for PageSize {
    fn default() -> Self {
        PageSize::A4
    }
}

impl<'de> Deserialize<'de> for PageSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum PageSizeDef {
            Str(String),
            Map { width: f32, height: f32 },
        }

        match PageSizeDef::deserialize(deserializer)? {
            PageSizeDef::Str(s) => crate::parser::style::parse_page_size(&s).map_err(de::Error::custom),
            PageSizeDef::Map { width, height } => Ok(PageSize::Custom { width, height }),
        }
    }
}