// src/pipeline/strategy/mod.rs
use crate::core::layout::FontManager;
use crate::parser::processor::CompiledTemplate;
use std::sync::Arc;

pub mod two_pass;

/// A container for all shared, read-only resources needed during a pipeline run.
/// This will be created once and passed to the chosen strategy.
#[derive(Clone)]
pub struct PipelineContext {
    pub compiled_template: Arc<dyn CompiledTemplate>,
    pub font_manager: Arc<FontManager>,
}