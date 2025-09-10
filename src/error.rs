use std::num::ParseFloatError;
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
}

/// A comprehensive error type for the entire document generation pipeline.
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("JSON parsing error: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("XML parsing error: {0}")]
    XmlParseError(#[from] quick_xml::Error),

    #[error("XML attribute parsing error: {0}")]
    XmlAttrError(#[from] quick_xml::events::attributes::AttrError),

    #[error("Failed to parse number in template: {0}")]
    FloatParseError(#[from] ParseFloatError),

    #[error("Template parsing error: {0}")]
    TemplateParseError(String),

    #[error("Template rendering error (e.g., Handlebars): {0}")]
    TemplateRenderError(String),

    #[error("Unknown XML tag found in template: <{0}>")]
    UnknownXmlTag(String),

    #[error("Stylesheet is invalid or missing required parts: {0}")]
    StylesheetError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Rendering failed: {0}")]
    RenderError(#[from] RenderError),
}