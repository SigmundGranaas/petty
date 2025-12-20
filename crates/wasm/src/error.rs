//! Error handling for WASM bindings.
//!
//! Converts Petty's error types into JavaScript-friendly errors.

use petty_core::error::PipelineError;
use wasm_bindgen::prelude::*;

/// Error codes for TypeScript consumption.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Configuration error (invalid template, missing settings)
    Config,
    /// I/O error (file read/write failures)
    Io,
    /// Template parsing error
    Parse,
    /// Rendering error
    Render,
    /// Layout error
    Layout,
    /// JSON serialization/deserialization error
    Json,
    /// Template execution error
    TemplateExecution,
    /// PDF processing error
    Pdf,
    /// Font-related error
    Font,
    /// Resource loading error
    Resource,
    /// Unknown error
    Unknown,
}

/// A JavaScript-friendly error type.
///
/// Note: This is NOT a wasm_bindgen struct because we need custom
/// conversion to JavaScript Error objects.
#[derive(Debug)]
pub struct PettyError {
    code: ErrorCode,
    message: String,
}

impl PettyError {
    /// Get the error code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Get the error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl PettyError {
    /// Create a new error with the given code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Create a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Config, message)
    }

    /// Create a font error.
    pub fn font(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Font, message)
    }

    /// Create a resource error.
    pub fn resource(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Resource, message)
    }
}

impl From<PipelineError> for PettyError {
    fn from(err: PipelineError) -> Self {
        let (code, message) = match &err {
            PipelineError::Config(msg) => (ErrorCode::Config, msg.clone()),
            PipelineError::Io(e) => (ErrorCode::Io, e.to_string()),
            PipelineError::StylesheetError(msg) => (ErrorCode::Parse, msg.clone()),
            PipelineError::Parse(e) => (ErrorCode::Parse, e.to_string()),
            PipelineError::Render(msg) => (ErrorCode::Render, msg.clone()),
            PipelineError::Layout(msg) => (ErrorCode::Layout, msg.clone()),
            PipelineError::Json(e) => (ErrorCode::Json, e.to_string()),
            PipelineError::TemplateExecution(msg) => (ErrorCode::TemplateExecution, msg.clone()),
            PipelineError::Pdf(msg) => (ErrorCode::Pdf, msg.clone()),
            PipelineError::Other(msg) => (ErrorCode::Unknown, msg.clone()),
        };

        Self { code, message }
    }
}

impl From<PettyError> for JsValue {
    fn from(err: PettyError) -> Self {
        let js_error = js_sys::Error::new(&err.message);

        // Add the error code as a property
        let code_str = match err.code {
            ErrorCode::Config => "CONFIG_ERROR",
            ErrorCode::Io => "IO_ERROR",
            ErrorCode::Parse => "PARSE_ERROR",
            ErrorCode::Render => "RENDER_ERROR",
            ErrorCode::Layout => "LAYOUT_ERROR",
            ErrorCode::Json => "JSON_ERROR",
            ErrorCode::TemplateExecution => "TEMPLATE_EXECUTION_ERROR",
            ErrorCode::Pdf => "PDF_ERROR",
            ErrorCode::Font => "FONT_ERROR",
            ErrorCode::Resource => "RESOURCE_ERROR",
            ErrorCode::Unknown => "UNKNOWN_ERROR",
        };

        js_sys::Reflect::set(&js_error, &"code".into(), &JsValue::from_str(code_str)).ok();

        js_error.into()
    }
}

impl From<serde_json::Error> for PettyError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorCode::Json, err.to_string())
    }
}

impl From<petty_traits::FontError> for PettyError {
    fn from(err: petty_traits::FontError) -> Self {
        Self::new(ErrorCode::Font, err.to_string())
    }
}

impl From<petty_traits::ResourceError> for PettyError {
    fn from(err: petty_traits::ResourceError) -> Self {
        Self::new(ErrorCode::Resource, err.to_string())
    }
}
