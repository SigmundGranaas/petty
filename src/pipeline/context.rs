use crate::core::layout::fonts::SharedFontLibrary;
use crate::parser::processor::CompiledTemplate;
use crate::pipeline::config::PipelineCacheConfig;
use std::collections::HashMap;
use std::sync::Arc;

/// A container for all shared, read-only resources needed during a pipeline run.
/// This will be created once by the `PipelineBuilder` and passed to the various
/// pipeline components.
#[derive(Clone)]
pub struct PipelineContext {
    pub compiled_template: Arc<dyn CompiledTemplate>,
    pub role_templates: Arc<HashMap<String, Arc<dyn CompiledTemplate>>>,
    pub font_library: Arc<SharedFontLibrary>,
    pub cache_config: PipelineCacheConfig,
}