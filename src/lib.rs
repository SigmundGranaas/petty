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
//!
//! ## Feature Flags
//!
//! Petty supports optional features that can be enabled in your `Cargo.toml`:
//!
//! ### `parallel-render`
//!
//! Enables parallel PDF content rendering using Rayon. This can significantly improve
//! throughput when generating multi-page documents by parallelizing page content
//! stream generation.
//!
//! ```toml
//! [dependencies]
//! petty = { version = "...", features = ["parallel-render"] }
//! ```
//!
//! When enabled, use `pipeline::configure_rayon_pool()` to customize the thread pool size:
//!
//! ```ignore
//! use petty::pipeline::configure_rayon_pool;
//!
//! // Use half of available CPUs for PDF rendering
//! configure_rayon_pool(num_cpus::get() / 2);
//! ```
//!
//! ## Worker Configuration
//!
//! The number of layout worker threads can be configured in order of priority:
//!
//! 1. Explicit configuration via [`PipelineBuilder::with_worker_count`]
//! 2. The `PETTY_WORKER_COUNT` environment variable
//! 3. Auto-detection based on CPU count (`num_cpus - 1`, minimum 2)
//!
//! ### Adaptive Scaling (Experimental)
//!
//! The [`pipeline::adaptive`] module provides experimental support for dynamic worker
//! scaling based on workload. Enable with [`PipelineBuilder::with_adaptive_scaling`]:
//!
//! ```ignore
//! let pipeline = PipelineBuilder::new()
//!     .with_template_file("template.json")?
//!     .with_adaptive_scaling(true)
//!     .build()?;
//! ```
//!
//! See [`AdaptiveController`](pipeline::AdaptiveController) and
//! [`WorkerManager`](pipeline::WorkerManager) for lower-level control.

// ============================================================================
// Foundation Crates - Basic types and abstractions
// ============================================================================

/// Intermediate Document Format - IR nodes representing document structure
pub use petty_idf as idf;
/// Styling system - CSS-like styles and dimensions
pub use petty_style as style;
/// Core traits for extensibility (Executor, FontProvider, ResourceProvider)
pub use petty_traits as traits;
/// Base types (geometry, colors, document metadata)
pub use petty_types as types_base;

// ============================================================================
// Algorithm Crates - Data querying and layout
// ============================================================================

/// JSONPath implementation for data querying
pub use petty_jpath as jpath;
/// Layout engine - converts IR to positioned elements
pub use petty_layout as layout;
/// XPath 1.0 implementation for data querying
pub use petty_xpath1 as xpath;

// ============================================================================
// Template Crates - Template parsing and compilation
// ============================================================================

/// JSON template parser and executor
pub use petty_json_template as json_template;
/// Template core abstractions (shared by all template engines)
pub use petty_template_core as template_core;
/// XSLT 1.0 parser and executor
pub use petty_xslt as xslt;

// ============================================================================
// Render Crates - PDF generation
// ============================================================================

/// PDF document composition utilities
pub use petty_pdf_composer as pdf_composer;
/// Core rendering abstractions and types
pub use petty_render_core as render_core;
/// lopdf-based PDF renderer implementation
pub use petty_render_lopdf as render_lopdf;

// ============================================================================
// Platform Crates - Platform-specific functionality
// ============================================================================

/// Execution strategies (single-threaded, multi-threaded, async)
pub use petty_executor as executor;
/// Resource loading and caching
pub use petty_resource as resource;
/// Data source abstractions
pub use petty_source as source;
/// Fluent builder API for programmatic document creation
pub use petty_template_dsl as templating;

// ============================================================================
// Core Integration Layer
// ============================================================================

/// Integration layer combining all subsystems
pub use petty_core as core_internal;
pub use petty_core::core;
pub use petty_core::error;
pub use petty_core::parser;
pub use petty_core::types;

// ============================================================================
// High-Level API - Most commonly used types
// ============================================================================

// Errors
pub use petty_core::PipelineError;

// Document types
pub use layout::LaidOutSequence;
pub use petty_core::{ApiIndexEntry, TocEntry};

// IR types
pub use idf::{IRNode, InlineNode};

// Style types
pub use style::{Dimension, ElementStyle, FontStyle, FontWeight};

// Layout types
pub use layout::{LayoutEngine, LayoutStore, Paginator, PositionedElement};

// Template types
pub use template_core::{
    CompiledTemplate, DataSourceFormat, ExecutionConfig, TemplateExecutor, TemplateFeatures,
    TemplateFlags, TemplateMetadata, TemplateParser,
};

// Geometry and colors
pub use types_base::{AnchorId, BoxConstraints, Color, IndexTerm, Rect, ResourceUri, Size};

// Traits for extensibility
pub use traits::{Executor, FontProvider, ResourceProvider};

// Pipeline module (orchestration layer - stays in main crate)
pub mod pipeline;

// Public API
pub use crate::pipeline::{
    DocumentPipeline, GenerationMode, PdfBackend, PipelineBuilder, ProcessingMode,
};

// Helper trait for error conversion
pub(crate) trait MapRenderError<T> {
    fn map_render_err(self) -> Result<T, PipelineError>;
}

impl<T> MapRenderError<T> for Result<T, render_core::RenderError> {
    fn map_render_err(self) -> Result<T, PipelineError> {
        self.map_err(PipelineError::Render)
    }
}

// Helper trait for ComposerError conversion
pub(crate) trait MapComposerError<T> {
    fn map_composer_err(self) -> Result<T, PipelineError>;
}

impl<T> MapComposerError<T> for Result<T, pdf_composer::ComposerError> {
    fn map_composer_err(self) -> Result<T, PipelineError> {
        self.map_err(|e| match e {
            pdf_composer::ComposerError::Pdf(lopdf_err) => PipelineError::Pdf(lopdf_err),
            pdf_composer::ComposerError::Other(msg) => PipelineError::Other(msg),
        })
    }
}
