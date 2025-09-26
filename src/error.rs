// src/error.rs
use thiserror::Error;
use crate::core::layout::LayoutError;
use crate::parser::ParseError;
use crate::render::RenderError;

/// A comprehensive error type for the entire document generation pipeline.
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Parsing failed: {0}")]
    Parse(#[from] ParseError),

    #[error("Layout failed: {0}")]
    Layout(#[from] LayoutError),

    #[error("Rendering failed: {0}")]
    Render(#[from] RenderError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Stylesheet is invalid or missing required parts: {0}")]
    StylesheetError(String),
}

// Add this implementation to handle JSON errors at the top level (e.g., in examples)
impl From<serde_json::Error> for PipelineError {
    fn from(e: serde_json::Error) -> Self {
        PipelineError::Parse(ParseError::JsonParse(e))
    }
}