// FILE: /home/sigmund/RustroverProjects/petty/src/parser/processor.rs
use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use crate::parser::ParseError;
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
    /// If true, enables strict compliance checks. For example, referencing an
    /// undeclared variable or calling a template with an undeclared parameter
    /// will result in an error instead of default behavior.
    pub strict: bool,
}

/// A reusable, data-agnostic, compiled template artifact.
///
/// This trait represents the result of the "compilation" phase. An object
/// implementing this trait can be stored and reused to execute the template against
/// multiple different data sources, avoiding the cost of re-parsing the template file.
pub trait CompiledTemplate: Send + Sync {
    /// Executes the template against a data context to produce a self-contained IRNode tree.
    ///
    /// # Arguments
    ///
    /// * `data_source`: A string slice containing the data (e.g., XML or JSON) to transform.
    /// * `config`: An `ExecutionConfig` specifying the data format and other runtime options.
    ///
    /// # Returns
    ///
    /// A `Result` containing either the root-level nodes of the generated document tree
    /// or a `PipelineError` if execution fails.
    fn execute(&self, data_source: &str, config: ExecutionConfig) -> Result<Vec<IRNode>, PipelineError>;

    /// Returns a reference to the stylesheet containing resolved styles and page masters
    /// associated with this template.
    fn stylesheet(&self) -> &Stylesheet;

    /// Returns the base path for resolving relative resource paths (e.g., for images).
    /// This path is typically the directory where the original template file was located.
    fn resource_base_path(&self) -> &Path;
}

/// A parser responsible for compiling a template string into a `CompiledTemplate`.
///
/// This trait defines the public interface for different template language parsers
/// (e.g., `XsltParser`, `JsonParser`).
pub trait TemplateParser {
    /// Parses a template source string.
    ///
    /// # Arguments
    ///
    /// * `template_source`: The content of the template file (e.g., an XSLT stylesheet).
    /// * `resource_base_path`: The directory containing the template file, used to resolve
    ///   relative paths for resources like images.
    ///
    /// # Returns
    ///
    /// A `Result` containing a thread-safe `Arc` pointer to a `CompiledTemplate`
    /// or a `ParseError` if compilation fails.
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<Arc<dyn CompiledTemplate>, ParseError>;
}