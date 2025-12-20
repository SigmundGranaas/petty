//! # petty-core
//!
//! Platform-agnostic PDF generation core library.
//!
//! This crate provides the core functionality for PDF document generation:
//! - **core**: Primitive data types for styling, layout, and document structure
//! - **parser**: Template parsing (XSLT and JSON formats)
//! - **render**: PDF rendering using the `Write` trait
//! - **types**: Shared data types bridging layout and render phases
//! - **error**: Error types for the pipeline
//!
//! ## Design Principle
//!
//! This crate has **no platform dependencies**:
//! - No filesystem access (`std::fs`)
//! - No system font discovery (`fontdb`)
//! - No async runtime (`tokio`)
//! - No threading primitives beyond `Send + Sync`
//!
//! All platform-specific functionality is provided through traits that
//! implementors can fulfill for their target platform.

// Re-export foundation crates
pub use petty_idf as idf;
pub use petty_style as style_types;
pub use petty_traits as traits;
pub use petty_types as types_base;

// Re-export algorithm crates
pub use petty_layout as layout;
pub use petty_xpath1 as xpath;

pub mod core;
pub mod error;
pub mod parser;
pub mod types;

// Re-export commonly used types from foundation crates
pub use idf::{IRNode, InlineNode, NodeMetadata};
pub use style_types::{Border, Dimension, ElementStyle, FontStyle, FontWeight, Margins};
pub use types_base::{BoxConstraints, Color, Rect, Size};

// Re-export from internal modules
pub use error::PipelineError;
pub use types::{ApiIndexEntry, LaidOutSequence, TocEntry};

// Re-export platform abstraction traits
pub use traits::{
    Executor, ExecutorError, FontDescriptor, FontError, FontProvider, FontQuery,
    InMemoryFontProvider, InMemoryResourceProvider, ResourceError, ResourceProvider,
    SharedFontData, SharedResourceData, SyncExecutor,
};
