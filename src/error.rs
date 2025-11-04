// src/error.rs
use crate::parser::ParseError;
use crate::render::RenderError;
use thiserror::Error;

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
    Layout(String),
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Template execution error: {0}")]
    TemplateExecution(String),
    #[error("Other pipeline error: {0}")]
    Other(String),
}

impl From<crate::core::layout::LayoutError> for PipelineError {
    fn from(e: crate::core::layout::LayoutError) -> Self {
        PipelineError::Layout(e.to_string())
    }
}