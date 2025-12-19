//! Defines primitives for size, position, and spacing.
use crate::parser::style_parsers;
use serde::{de, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use std::hash::{Hash, Hasher};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Dimension {
    Pt(f32),
    Percent(f32),
    Auto,
}

impl Hash for Dimension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Dimension::Pt(v) => {
                0u8.hash(state);
                v.to_bits().hash(state);
            }
            Dimension::Percent(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
            Dimension::Auto => {
                2u8.hash(state);
            }
        }
    }
}

impl Eq for Dimension {}

impl Default for Dimension {
    fn default() -> Self {
        Dimension::Auto
    }
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Hash for Margins {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.top.to_bits().hash(state);
        self.right.to_bits().hash(state);
        self.bottom.to_bits().hash(state);
        self.left.to_bits().hash(state);
    }
}

impl Eq for Margins {}

impl Margins {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
    pub fn x(value: f32) -> Self {
        Self {
            top: 0f32,
            right: value,
            bottom: 0f32,
            left: value,
        }
    }
    pub fn y(value: f32) -> Self {
        Self {
            top: value,
            right: 0f32,
            bottom: value,
            left: 0f32,
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

#[derive(Debug, Clone, PartialEq)]
pub enum PageSize {
    A4,
    Letter,
    Legal,
    Custom { width: f32, height: f32 },
}

impl Eq for PageSize {}

impl Hash for PageSize {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            PageSize::A4 => 0u8.hash(state),
            PageSize::Letter => 1u8.hash(state),
            PageSize::Legal => 2u8.hash(state),
            PageSize::Custom { width, height } => {
                3u8.hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
            }
        }
    }
}

impl Serialize for PageSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            PageSize::A4 => serializer.serialize_str("A4"),
            PageSize::Letter => serializer.serialize_str("Letter"),
            PageSize::Legal => serializer.serialize_str("Legal"),
            PageSize::Custom { width, height } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("width", width)?;
                map.serialize_entry("height", height)?;
                map.end()
            }
        }
    }
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