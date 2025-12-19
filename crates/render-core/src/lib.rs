//! Core rendering abstractions for PDF generation.
//!
//! This crate provides the fundamental traits and types used by PDF rendering backends:
//! - `DocumentRenderer` trait for abstracting PDF writing operations
//! - Error types for rendering operations
//! - Shared utility functions for font handling and coordinate conversion

mod error;
mod traits;
mod types;
pub mod utils;

pub use error::RenderError;
pub use traits::DocumentRenderer;
pub use types::{LaidOutSequence, Pass1Result, ResolvedAnchor, HyperlinkLocation};
