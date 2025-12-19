//! # Petty
//!
//! High-performance PDF generation engine with pluggable executors and resource providers.
//!
//! ## Module Structure
//!
//! - `core`: Re-exported from `petty-core` - primitive data types for styling and layout
//! - `parser`: Re-exported from `petty-core` - template parsing (XSLT and JSON)
//! - `render`: Re-exported from `petty-core` - PDF rendering
//! - `pipeline`: Document generation orchestration (platform-specific)
//! - `templating`: Fluent builder API for creating documents programmatically
//!
//! ## Usage
//!
//! ```ignore
//! use petty::{PipelineBuilder, PipelineError};
//!
//! let pipeline = PipelineBuilder::new()
//!     .with_template_file("template.xslt")?
//!     .build()?;
//!
//! pipeline.generate_to_file(vec![data], "output.pdf")?;
//! ```

// Re-export core modules from petty-core
pub use petty_core::core;
pub use petty_core::parser;
pub use petty_core::render;
pub use petty_core::error;
pub use petty_core::types;
pub use petty_core::PipelineError;
pub use petty_core::{LaidOutSequence, TocEntry, ApiIndexEntry};

// Platform-specific modules (remain in this crate)
pub mod executor;
mod pipeline;
pub mod resource;
pub mod source;
pub mod templating;

// Public API
pub use crate::pipeline::{PdfBackend, PipelineBuilder, GenerationMode};
