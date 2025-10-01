use crate::core::idf::LayoutUnit;
use crate::error::PipelineError;
use serde_json::Value;

/// A trait for processing a template and data into a stream of `LayoutUnit`s.
///
/// This abstraction allows multiple templating languages (e.g., JSON, XSLT) to be
/// used interchangeably by the document generation pipeline. An implementation's `process`
/// method is responsible for parsing the input template against the provided data and
/// producing an iterator, where each item represents one complete `sequence` tree.
pub trait TemplateProcessor: Send {
    /// Processes the template against the given data source.
    ///
    /// # Arguments
    /// * `data` - The source `serde_json::Value` to use for templating.
    ///
    /// # Returns
    /// A `Result` containing a boxed iterator that yields `LayoutUnit`s. This design
    /// supports the engine's streaming architecture by allowing the parser to lazily
    /// produce document chunks (`sequence`s) as they are needed by the layout engine,
    /// keeping memory usage low and predictable.
    fn process<'a>(
        &'a mut self,
        data: &'a Value,
    ) -> Result<Box<dyn Iterator<Item = Result<LayoutUnit, PipelineError>> + 'a + Send>, PipelineError>;
}