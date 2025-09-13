// src/layout/mod.rs

//! The tree-based, multi-pass layout engine.

// Re-export the main entry point and key types for external use.
pub use self::engine::LayoutEngine;

// Declare the modules that make up the layout engine.
mod engine;
mod page;
mod style;

// Sub-modules for laying out specific element types.
mod block;
mod elements;
mod flex;
mod image;
mod table;
mod text;


// Publicly expose types that are needed for the layout process but defined elsewhere.
// This prevents other parts of the application from needing to know the internal
// structure of the layout module.
pub use crate::error::PipelineError;
pub use crate::idf::{IRNode, InlineNode, LayoutUnit};
pub use self::elements::{
    ImageElement, LayoutElement, PositionedElement, TextElement,
};
pub use crate::stylesheet::Stylesheet;

/// A work item for the layout stack in the positioning pass.
/// It represents either a node to be laid out or a marker to end a node's context
/// (e.g., to apply bottom margin).
#[derive(Clone)]
enum WorkItem {
    Node(IRNode),
    EndNode(style::ComputedStyle),
}