use thiserror::Error;

#[derive(Error, Debug)]
pub enum LayoutError {
    #[error("Node has a height of {0:.2} which exceeds the total page content height of {1:.2}.")]
    ElementTooLarge(f32, f32),
    #[error("Builder mismatch: Expected {0} node, got {1}.")]
    BuilderMismatch(&'static str, &'static str),
    #[error("State mismatch: Expected state for {0}, got {1}.")]
    StateMismatch(&'static str, &'static str),
    #[error("Generic layout error: {0}")]
    Generic(String),
}

// New modules from refactor
pub mod cache;

// Existing modules
pub(crate) mod engine;
pub use self::engine::{LayoutEngine, LayoutStore};
pub use self::interface::{AnchorLocation, IndexEntry, LayoutContext, LayoutEnvironment, NodeState};

mod elements;
pub mod builder;
pub mod config;
pub mod fonts;
pub mod interface;
pub mod node_kind;
pub mod nodes;
pub mod perf;
pub mod style;
pub mod util;

pub mod algorithms;
pub mod painting;
pub mod text;

// Re-exports for convenience within the layout crate
pub use self::elements::{ImageElement, LayoutElement, PositionedElement, TextElement};
pub use self::fonts::{FontFaceInfo, SharedFontLibrary};
pub use self::style::ComputedStyle;
pub use self::config::LayoutConfig;

// Re-export geometry types used by nodes from base to prevent type mismatches
pub use petty_types::geometry::{BoxConstraints, Size, Rect};

// Re-export interface types used by nodes
pub use self::interface::{
    LayoutResult, LayoutNode,
    BlockState, FlexState, ListItemState, ParagraphState, TableState
};

#[cfg(test)]
mod integration_test;
#[cfg(test)]
mod list_test;
#[cfg(test)]
mod style_test;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod text_test;