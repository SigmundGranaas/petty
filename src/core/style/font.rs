// src/core/font.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum FontWeight {
    Thin,
    Light,
    Regular,
    Medium,
    Bold,
    Black,
    #[serde(untagged)]
    Numeric(u16),
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::Regular
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle::Normal
    }
}