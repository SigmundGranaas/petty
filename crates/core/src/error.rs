// src/error.rs
//! Defines the unified, rich error types for all pipeline operations.

use crate::layout::LayoutError;
use crate::parser::ParseError;
use petty_render_core::RenderError;
use petty_template_core::TemplateError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("XPath evaluation failed: {0}")]
    XPath(String),
    #[error("Call to unknown named template: '{0}'")]
    UnknownNamedTemplate(String),
    #[error("Error in function '{function}': {message}")]
    FunctionError { function: String, message: String },
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Template error: {0}")]
    Template(#[from] TemplateError),
}

/// The main error enum for all high-level operations within the engine.
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Stylesheet parsing or processing error: {0}")]
    StylesheetError(String),
    #[error("Template parsing error: {0}")]
    Parse(#[from] ParseError),
    #[error("Rendering error: {0}")]
    Render(#[from] RenderError),
    #[error("Layout error: {0}")]
    Layout(#[from] LayoutError),
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Template execution error: {0}")]
    TemplateExecution(#[from] ExecutionError),
    #[error("PDF processing error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("Other pipeline error: {0}")]
    Other(String),
}

// Add a direct conversion from TemplateError to PipelineError
impl From<TemplateError> for PipelineError {
    fn from(e: TemplateError) -> Self {
        PipelineError::TemplateExecution(ExecutionError::Template(e))
    }
}
