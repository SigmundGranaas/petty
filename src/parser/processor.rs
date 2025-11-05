// src/parser/processor.rs
use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Specifies the format of the input data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSourceFormat {
    Xml,
    Json,
}

impl Default for DataSourceFormat {
    fn default() -> Self {
        DataSourceFormat::Xml
    }
}

/// Configuration for an execution run.
#[derive(Debug, Clone, Default)]
pub struct ExecutionConfig {
    /// The format of the data source string.
    pub format: DataSourceFormat,
    /// If true, enables strict compliance checks.
    pub strict: bool,
}

/// A struct to report features found in a template that may require special handling.
#[derive(Debug, Default, Clone, Copy)]
pub struct TemplateFeatures {
    /// True if the template contains a `<toc>` element or equivalent.
    pub has_table_of_contents: bool,
    /// True if the template contains a "page X of Y" placeholder.
    pub has_page_number_placeholders: bool,
}

/// A reusable, data-agnostic, compiled template artifact.
pub trait CompiledTemplate: Send + Sync {
    /// Executes the template against a data context to produce a self-contained IRNode tree.
    fn execute(&self, data_source: &str, config: ExecutionConfig) -> Result<Vec<IRNode>, PipelineError>;

    /// Returns a shared pointer to the stylesheet.
    fn stylesheet(&self) -> Arc<Stylesheet>;

    /// Returns the base path for resolving relative resource paths.
    fn resource_base_path(&self) -> &Path;

    /// Returns a summary of features detected in the template.
    fn features(&self) -> TemplateFeatures;
}

/// A parser responsible for compiling a template string into a `CompiledTemplate`.
pub trait TemplateParser {
    /// Parses a template source string.
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<Arc<dyn CompiledTemplate>, PipelineError>;
}