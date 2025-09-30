//! Defines enums for CSS List properties.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ListStyleType {
    Disc,
    Circle,
    Square,
    Decimal,
    None,
}

impl Default for ListStyleType {
    fn default() -> Self {
        ListStyleType::Disc
    }
}