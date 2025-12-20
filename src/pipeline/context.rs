use crate::pipeline::adaptive::{AdaptiveController, AdaptiveScalingFacade, WorkerManager};
use crate::pipeline::config::PipelineCacheConfig;
use petty_core::layout::fonts::SharedFontLibrary;
use petty_core::parser::processor::CompiledTemplate;
use petty_core::traits::ResourceProvider;
use std::collections::HashMap;
use std::sync::Arc;

/// A container for all shared, read-only resources needed during a pipeline run.
/// This will be created once by the `PipelineBuilder` and passed to the various
/// pipeline components.
///
/// The context holds trait objects for platform-agnostic abstractions:
/// - `ResourceProvider`: Loads images and other external resources
/// - `AdaptiveScalingFacade`: Optional facade for metrics and adaptive scaling
///
/// # Adaptive Scaling
///
/// When the `adaptive` field is set, workers will record processing times and
/// the consumer will track queue depth. With the `adaptive-scaling` feature,
/// dynamic worker scaling is also available.
#[derive(Clone)]
pub struct PipelineContext {
    pub compiled_template: Arc<dyn CompiledTemplate>,
    pub role_templates: Arc<HashMap<String, Arc<dyn CompiledTemplate>>>,
    pub font_library: Arc<SharedFontLibrary>,
    pub resource_provider: Arc<dyn ResourceProvider>,
    pub cache_config: PipelineCacheConfig,
    /// Optional adaptive scaling facade for metrics collection and dynamic scaling.
    /// Replaces the separate `adaptive_controller` and `worker_manager` fields.
    pub adaptive: Option<Arc<AdaptiveScalingFacade>>,
}

impl PipelineContext {
    /// Get the adaptive controller if available.
    ///
    /// This is a convenience method that extracts the controller from the facade.
    pub fn adaptive_controller(&self) -> Option<Arc<AdaptiveController>> {
        self.adaptive.as_ref().map(|f| Arc::clone(f.controller()))
    }

    /// Get the worker manager if available.
    pub fn worker_manager(&self) -> Option<Arc<WorkerManager>> {
        self.adaptive.as_ref().map(|f| Arc::clone(f.manager()))
    }
}
