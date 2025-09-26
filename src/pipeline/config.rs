// FILE: src/pipeline/config.rs
// src/pipeline/config.rs
use crate::parser::xslt::ast::CompiledStylesheet;
use std::path::PathBuf;
use std::sync::Arc;

/// An enum to select the desired PDF rendering backend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PdfBackend {
    /// In-memory PDF generation using `printpdf`.
    PrintPdf,
    /// In-memory PDF generation using `printpdf`, with parallel page rendering.
    PrintPdfParallel,
    #[default]
    /// Streaming PDF generation using `lopdf`.
    Lopdf,
    /// Streaming PDF generation using `lopdf`, with parallel page rendering.
    LopdfParallel,
}

/// A fully pre-compiled XSLT stylesheet and its associated metadata.
#[derive(Clone)]
pub struct XsltTemplate {
    pub(crate) compiled_stylesheet: CompiledStylesheet,
    pub(crate) resource_base_path: PathBuf,
}

/// A JSON template and its associated metadata.
#[derive(Clone)]
pub struct JsonTemplate {
    pub(crate) template_content: Arc<String>,
    pub(crate) resource_base_path: PathBuf,
}

/// An enum representing a configured template of any supported language.
#[derive(Clone)]
pub enum Template {
    Xslt(XsltTemplate),
    Json(JsonTemplate),
}