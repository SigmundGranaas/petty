use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::render::DocumentRenderer;
use serde_json::Value;

/// A trait for processing a template and data into a stream of layout events.
/// This allows for multiple templating languages (e.g., JSON, XSLT) to be
/// used interchangeably by the document generation pipeline.
pub trait TemplateProcessor<'a> {
    /// Processes the given data with its configured template, feeding `Event`s
    /// to the layout processor.
    ///
    /// # Arguments
    /// * `data` - The source `serde_json::Value` to use for templating.
    /// * `processor` - The layout processor that will consume the generated events.
    fn process<R: DocumentRenderer<'a>>(
        &mut self,
        data: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError>;
}