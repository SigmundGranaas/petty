use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use crate::parser::ParseError;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A reusable, data-agnostic, compiled template artifact.
pub trait CompiledTemplate: Send + Sync {
    /// Executes the template against a data context to produce a self-contained IRNode tree.
    fn execute(&self, data: &Value) -> Result<Vec<IRNode>, PipelineError>;

    /// Returns the stylesheet containing resolved styles and page masters.
    fn stylesheet(&self) -> &Stylesheet;

    /// Returns the base path for resolving relative resource paths (e.g., images).
    fn resource_base_path(&self) -> &Path;
}

/// A parser responsible for compiling a template string into a `CompiledTemplate`.
pub trait TemplateParser {
    /// Parses a template source string and its resource base path.
    fn parse(&self, template_source: &str, resource_base_path: PathBuf) -> Result<Arc<dyn CompiledTemplate>, ParseError>;
}