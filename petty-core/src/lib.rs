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

pub mod core;
pub mod error;
pub mod parser;
pub mod render;
pub mod traits;
pub mod types;

// Re-export commonly used types
pub use error::PipelineError;
pub use types::{LaidOutSequence, TocEntry, ApiIndexEntry};

// Re-export platform abstraction traits
pub use traits::{
    Executor, ExecutorError, SyncExecutor,
    FontProvider, FontError, FontQuery, FontDescriptor, InMemoryFontProvider, SharedFontData,
    ResourceProvider, ResourceError, InMemoryResourceProvider, SharedResourceData,
};
