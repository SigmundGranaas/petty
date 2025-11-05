// src/pipeline/strategy/mod.rs
mod single_pass;
mod two_pass;
mod hybrid_buffered;

pub use single_pass::SinglePassStreamingStrategy;
pub use two_pass::TwoPassStrategy;
pub use hybrid_buffered::HybridBufferedStrategy;

use crate::core::layout::FontManager;
use crate::parser::processor::CompiledTemplate;
use std::sync::Arc;

/// A container for all shared, read-only resources needed during a pipeline run.
/// This will be created once and passed to the chosen strategy.
#[derive(Clone)]
pub struct PipelineContext {
    pub compiled_template: Arc<dyn CompiledTemplate>,
    pub font_manager: Arc<FontManager>,
}

/// An enum representing the high-level algorithm for generating a document.
///
/// This enum-based approach is used instead of a trait object (`dyn Trait`)
/// because the different strategies have different requirements for the data
/// iterator (e.g., `TwoPassStrategy` requires `I: Clone`), which cannot be
/// expressed in an object-safe trait.
#[derive(Clone)]
pub enum GenerationStrategy {
    SinglePass(SinglePassStreamingStrategy),
    TwoPass(TwoPassStrategy),
    Hybrid(HybridBufferedStrategy),
}