use thiserror::Error;

#[derive(Error, Debug)]
pub enum LayoutError {
    #[error("Node has a height of {0:.2} which exceeds the total page content height of {1:.2}.")]
    ElementTooLarge(f32, f32),
}
pub type Result<T> = std::result::Result<T, LayoutError>;


/// The tree-based, multi-pass layout engine.

// Re-export the main entry point and key types for external use.
pub use self::engine::LayoutEngine;
pub use self::page::PageIterator;

// Declare the modules that make up the layout engine.
mod engine;
mod fonts;
mod page;
pub mod style;
mod subtree; // Only one declaration

// Sub-modules for laying out specific element types.
mod block;
mod elements;
mod flex;
mod image;
mod table;
mod text;


// Publicly expose types that are needed for the layout process but defined elsewhere.
pub use crate::error::PipelineError;
pub use self::elements::{
    ImageElement, LayoutElement, PositionedElement, TextElement,
};
pub use self::fonts::FontManager;
pub use self::style::ComputedStyle;
use std::sync::Arc;
use crate::core::idf::IRNode;

/// A work item for the layout stack in the positioning pass.
/// It represents either a node to be laid out or a marker to end a node's context
/// (e.g., to apply bottom margin).
#[derive(Clone)]
pub(crate) enum WorkItem {
    Node(IRNode),
    EndNode(Arc<ComputedStyle>),
    // NEW: Flexbox control items
    StartFlex(Vec<f32>), // Contains calculated widths of children
    EndFlex,
}


#[cfg(test)] // This should apply to the module itself, not just its declaration.
mod integration_test;
