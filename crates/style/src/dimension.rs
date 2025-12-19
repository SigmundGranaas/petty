//! Defines primitives for size, position, and spacing.
use serde::{de, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use std::hash::{Hash, Hasher};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum Dimension {
    Pt(f32),
    Percent(f32),
    #[default]
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

    /// Parse a CSS-style length value with optional unit (e.g., "10pt", "5mm", "12")
    fn parse_length(input: &str) -> Result<f32, String> {
        let input = input.trim();

        // Try to find unit suffix
        if let Some(val) = input.strip_suffix("pt") {
            return val.trim().parse::<f32>()
                .map_err(|e| format!("Invalid number: {}", e));
        }
        if let Some(val) = input.strip_suffix("px") {
            return val.trim().parse::<f32>()
                .map_err(|e| format!("Invalid number: {}", e));
        }
        if let Some(val) = input.strip_suffix("in") {
            return val.trim().parse::<f32>()
                .map(|v| v * 72.0)
                .map_err(|e| format!("Invalid number: {}", e));
        }
        if let Some(val) = input.strip_suffix("cm") {
            return val.trim().parse::<f32>()
                .map(|v| v * 28.35)
                .map_err(|e| format!("Invalid number: {}", e));
        }
        if let Some(val) = input.strip_suffix("mm") {
            return val.trim().parse::<f32>()
                .map(|v| v * 2.835)
                .map_err(|e| format!("Invalid number: {}", e));
        }

        // No unit, assume points
        input.parse::<f32>()
            .map_err(|e| format!("Invalid number: {}", e))
    }

    /// Parse CSS-style margin shorthand (1, 2, or 4 values)
    fn parse_shorthand(input: &str) -> Result<Self, String> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let mut values = Vec::new();

        for part in parts {
            values.push(Self::parse_length(part)?);
        }

        match values.len() {
            1 => Ok(Margins::all(values[0])),
            2 => Ok(Margins {
                top: values[0],
                right: values[1],
                bottom: values[0],
                left: values[1],
            }),
            4 => Ok(Margins {
                top: values[0],
                right: values[1],
                bottom: values[2],
                left: values[3],
            }),
            _ => Err(format!(
                "Invalid margin shorthand: expected 1, 2, or 4 values, got {}",
                values.len()
            )),
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
                Margins::parse_shorthand(value).map_err(E::custom)
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
#[derive(Default)]
pub enum PageSize {
    #[default]
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

    /// Parse a page size name (e.g., "A4", "Letter", "Legal")
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "a4" => Ok(PageSize::A4),
            "letter" => Ok(PageSize::Letter),
            "legal" => Ok(PageSize::Legal),
            _ => Err(format!("Unknown page size: {}", s)),
        }
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
            PageSizeDef::Str(s) => Self::parse(&s).map_err(de::Error::custom),
            PageSizeDef::Map { width, height } => Ok(PageSize::Custom { width, height }),
        }
    }
}
