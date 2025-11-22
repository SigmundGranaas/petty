use thiserror::Error;

#[derive(Error, Debug)]
pub enum LayoutError {
    #[error("Node has a height of {0:.2} which exceeds the total page content height of {1:.2}.")]
    ElementTooLarge(f32, f32),
}

/// The tree-based, multi-pass layout engine.

// Re-export the main entry point and key types for external use.
pub use self::engine::LayoutEngine;
pub use self::node::{AnchorLocation, IndexEntry, LayoutBuffer, LayoutEnvironment};

// Declare the modules that make up the layout engine.
mod elements;
mod engine;
pub mod fonts;
pub mod geom;
pub mod node;
pub mod nodes;
pub mod style;
pub mod text;

// Publicly expose types that are needed for the layout process but defined elsewhere.
pub use crate::error::PipelineError;
pub use self::elements::{ImageElement, LayoutElement, PositionedElement, TextElement};
pub use self::fonts::FontManager;
pub use self::style::ComputedStyle;
use crate::core::idf::IRNode;

#[cfg(test)]
mod integration_test;

// Add declarations for the new test modules
#[cfg(test)]
mod list_test;
#[cfg(test)]
mod style_test;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod text_test;