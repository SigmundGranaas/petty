
use thiserror::Error;

/// Errors that can occur during the layout process.
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

/// The tree-based, multi-pass layout engine.

// Re-export the main entry point and key types for external use.
pub use self::engine::LayoutEngine;
pub use self::node::{AnchorLocation, IndexEntry, LayoutContext, LayoutEnvironment, NodeState};

// Declare the modules that make up the layout engine.
mod elements;
mod engine;
pub mod fonts;
pub mod geom;
pub mod node;
pub mod node_kind;
pub mod nodes;
pub mod style;
pub mod text;
pub mod util;

// Publicly expose types that are needed for the layout process but defined elsewhere.
pub use self::elements::{ImageElement, LayoutElement, PositionedElement, TextElement};
pub use self::fonts::FontManager;
pub use self::style::ComputedStyle;
pub use crate::error::PipelineError;

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