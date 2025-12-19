use petty_core::core::layout::fonts::SharedFontLibrary;
use crate::executor::ExecutorImpl;
use petty_core::parser::processor::CompiledTemplate;
use crate::pipeline::config::PipelineCacheConfig;
use petty_core::traits::ResourceProvider;
use std::collections::HashMap;
use std::sync::Arc;

/// A container for all shared, read-only resources needed during a pipeline run.
/// This will be created once by the `PipelineBuilder` and passed to the various
/// pipeline components.
///
/// The context holds trait objects for platform-agnostic abstractions:
/// - `ResourceProvider`: Loads images and other external resources
/// - `ExecutorImpl`: Controls parallelism and task execution
#[derive(Clone)]
pub struct PipelineContext {
    pub compiled_template: Arc<dyn CompiledTemplate>,
    pub role_templates: Arc<HashMap<String, Arc<dyn CompiledTemplate>>>,
    pub font_library: Arc<SharedFontLibrary>,
    pub resource_provider: Arc<dyn ResourceProvider>,
    pub executor: ExecutorImpl,
    pub cache_config: PipelineCacheConfig,
}