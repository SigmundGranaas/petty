//! Shared data types that bridge layout and render phases.
//!
//! These types represent the output of the layout phase and are consumed
//! by the rendering phase.

// Re-export from foundation crate
pub use petty_types::{ApiIndexEntry, TocEntry};

// Re-export from render-core crate (moved to avoid circular dependencies)
pub use petty_render_core::LaidOutSequence;
