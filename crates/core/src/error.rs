// src/error.rs
//! Defines the unified, rich error types for all pipeline operations.

use crate::layout::LayoutError;
use crate::parser::ParseError;
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
    Render(String),
    #[error("Layout error: {0}")]
    Layout(String),
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Template execution error: {0}")]
    TemplateExecution(String),
    #[error("PDF processing error: {0}")]
    Pdf(String),
    #[error("Other pipeline error: {0}")]
    Other(String),
}

impl From<LayoutError> for PipelineError {
    fn from(e: LayoutError) -> Self {
        PipelineError::Layout(e.to_string())
    }
}

impl From<ExecutionError> for PipelineError {
    fn from(e: ExecutionError) -> Self {
        PipelineError::TemplateExecution(e.to_string())
    }
}

impl From<lopdf::Error> for PipelineError {
    fn from(e: lopdf::Error) -> Self {
        PipelineError::Pdf(e.to_string())
    }
}