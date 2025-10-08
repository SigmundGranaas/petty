// FILE: /home/sigmund/RustroverProjects/petty/src/core/style/text.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
}

impl Default for TextAlign {
    fn default() -> Self {
        TextAlign::Left
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TextDecoration {
    None,
    Underline,
    LineThrough,
}

impl Default for TextDecoration {
    fn default() -> Self {
        TextDecoration::None
    }
}