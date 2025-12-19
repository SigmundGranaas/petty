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

// Re-export foundation crates
pub use petty_idf as idf;
pub use petty_style as style;
pub use petty_traits as traits;
pub use petty_types as types_base;

// Re-export algorithm crates
pub use petty_jpath as jpath;
pub use petty_layout as layout;
pub use petty_xpath1 as xpath;

// Re-export parser crates
pub use petty_json_template as json_template;
pub use petty_xslt as xslt;

// Re-export render crates
pub use petty_pdf_composer as pdf_composer;
pub use petty_render_core as render_core;
pub use petty_render_lopdf as render_lopdf;

// Re-export core modules from petty-core
pub use petty_core as core_internal;
pub use petty_core::PipelineError;
pub use petty_core::core;
pub use petty_core::error;
pub use petty_core::parser;
pub use petty_core::types;
pub use petty_core::{ApiIndexEntry, LaidOutSequence, TocEntry};

// Convenience re-exports from foundation crates
pub use idf::{IRNode, InlineNode};
pub use style::{Dimension, ElementStyle, FontStyle, FontWeight};
pub use traits::{Executor, FontProvider, ResourceProvider};
pub use types_base::{BoxConstraints, Color, Rect, Size};

// Re-export platform crates
pub use petty_executor as executor;
pub use petty_resource as resource;
pub use petty_source as source;
pub use petty_template_dsl as templating;

// Pipeline module (orchestration layer - stays in main crate)
mod pipeline;

// Public API
pub use crate::pipeline::{GenerationMode, PdfBackend, PipelineBuilder};

// Helper trait for error conversion
pub(crate) trait MapRenderError<T> {
    fn map_render_err(self) -> Result<T, PipelineError>;
}

impl<T> MapRenderError<T> for Result<T, render_core::RenderError> {
    fn map_render_err(self) -> Result<T, PipelineError> {
        self.map_err(|e| PipelineError::Render(e.to_string()))
    }
}
