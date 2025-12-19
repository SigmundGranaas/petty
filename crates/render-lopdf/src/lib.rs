//! High-performance PDF renderer using lopdf.
//!
//! This crate provides a streaming PDF renderer implementation using the lopdf library
//! for efficient PDF generation with minimal memory usage.

mod renderer;
mod writer;
mod helpers;

pub use renderer::LopdfRenderer;
pub use writer::StreamingPdfWriter;
pub use helpers::*;
