use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum FontWeight {
    Thin,
    Light,
    #[default]
    Regular,
    Medium,
    Bold,
    Black,
    Numeric(u16),
}

impl FontWeight {
    /// Returns the numeric weight value (100-900 scale).
    ///
    /// Standard CSS font-weight values:
    /// - Thin: 100
    /// - Light: 300
    /// - Regular: 400
    /// - Medium: 500
    /// - Bold: 700
    /// - Black: 900
    pub fn numeric_value(&self) -> u16 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::Bold => 700,
            FontWeight::Black => 900,
            FontWeight::Numeric(n) => *n,
        }
    }

    /// Parse a font weight from a string (e.g., "bold", "400")
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "thin" => Ok(FontWeight::Thin),
            "light" => Ok(FontWeight::Light),
            "regular" | "normal" => Ok(FontWeight::Regular),
            "medium" => Ok(FontWeight::Medium),
            "bold" => Ok(FontWeight::Bold),
            "black" => Ok(FontWeight::Black),
            _ => {
                s.parse::<u16>()
                    .map(FontWeight::Numeric)
                    .map_err(|_| format!("Invalid font weight: '{}'", s))
            }
        }
    }
}


impl<'de> Deserialize<'de> for FontWeight {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum FontWeightDef {
            Str(String),
            Num(u16),
        }

        match FontWeightDef::deserialize(deserializer)? {
            FontWeightDef::Str(s) => Self::parse(&s).map_err(de::Error::custom),
            FontWeightDef::Num(n) => Ok(FontWeight::Numeric(n)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

