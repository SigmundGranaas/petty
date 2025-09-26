// src/render/mod.rs
use thiserror::Error;

/// Errors that can occur during the rendering phase (e.g., PDF generation).
#[derive(Error, Debug)]
pub enum RenderError {
    #[error("A fatal, unrecoverable error occurred in a previous step.")]
    Aborted,
    #[error("PDF internal error: {0}")]
    InternalPdfError(String),
    #[error("Template error during rendering (e.g., in a footer): {0}")]
    TemplateError(String),
    #[error("I/O error during finalization: {0}")]
    IoError(#[from] std::io::Error),
    #[error("PDF library error: {0}")]
    PdfLibError(String),
    #[error("Lopdf library error: {0}")]
    LopdfError(#[from] lopdf::Error),
    #[error("An unspecified error occurred: {0}")]
    Other(String),
}

impl From<handlebars::RenderError> for RenderError {
    fn from(e: handlebars::RenderError) -> Self {
        RenderError::TemplateError(e.to_string())
    }
}


mod drawing;
pub mod lopdf_renderer;
pub mod pdf;
pub mod renderer;
mod streaming_writer;

// Re-export the main renderer and the trait
pub use self::renderer::DocumentRenderer;