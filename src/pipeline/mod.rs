//! Document generation pipeline orchestration.
//!
//! This module contains the core pipeline components for PDF generation:
//!
//! - [`PipelineBuilder`]: Fluent builder for constructing document pipelines
//! - [`GenerationMode`]: Controls whether streaming or metadata pipeline is used
//! - [`PdfBackend`]: Selects the PDF rendering backend
//!
//! # Adaptive Scaling (Experimental)
//!
//! The [`adaptive`] module provides experimental support for dynamic worker
//! scaling based on workload. See [`AdaptiveController`] and [`WorkerManager`]
//! for details.
//!
//! # Example
//!
//! ```ignore
//! use petty::{PipelineBuilder, GenerationMode};
//!
//! let pipeline = PipelineBuilder::new()
//!     .with_template_file("template.json")?
//!     .with_worker_count(4)
//!     .build()?;
//!
//! pipeline.generate_to_file(data, "output.pdf")?;
//! ```

pub mod adapters;
pub mod adaptive;
pub mod api;
mod builder;
pub(crate) mod concurrency;
pub mod config;
pub mod context;
mod orchestrator;
pub mod provider;
pub mod renderer;
pub(crate) mod worker;

// Core public API
pub use builder::PipelineBuilder;
pub use config::{GenerationMode, PdfBackend, ProcessingMode};

// Adaptive scaling API
// Public API exports for adaptive scaling and metrics collection (always available)
#[allow(unused_imports)]
pub use adaptive::{
    AdaptiveConfig, AdaptiveController, AdaptiveMetrics, AdaptiveScalingFacade, WorkerManager,
};

// Re-export Rayon configuration for parallel rendering
#[cfg(feature = "parallel-render")]
#[allow(unused_imports)] // Public API - may not be used internally
pub use concurrency::configure_rayon_pool;
