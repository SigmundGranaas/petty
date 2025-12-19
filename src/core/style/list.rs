// FILE: /home/sigmund/RustroverProjects/petty/src/core/style/list.rs
//! Defines enums for CSS List properties.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ListStyleType {
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    None,
}

impl Default for ListStyleType {
    fn default() -> Self {
        ListStyleType::Disc
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ListStylePosition {
    Inside,
    Outside,
}

impl Default for ListStylePosition {
    fn default() -> Self {
        ListStylePosition::Outside
    }
}