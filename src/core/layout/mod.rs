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
mod fragment;

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
pub use self::fragment::{LayoutBox, LayoutContent, Rect};
pub use self::style::ComputedStyle;
use crate::core::idf::IRNode;


#[cfg(test)]
mod integration_test;

// Add declarations for the new test modules
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod page_test;
#[cfg(test)]
mod flex_test;
#[cfg(test)]
mod table_test;
#[cfg(test)]
mod text_test;

