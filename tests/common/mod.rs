pub mod fixtures;
pub mod pdf_assertions;

use lopdf::Document as LopdfDocument;
use petty::{GenerationMode, PipelineBuilder, PipelineError};
use serde_json::Value;
use std::io::Cursor;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Wrapper around a generated PDF with helper methods
pub struct GeneratedPdf {
    pub bytes: Vec<u8>,
    pub doc: LopdfDocument,
}

impl GeneratedPdf {
    /// Create a GeneratedPdf from raw bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Box<dyn std::error::Error>> {
        let doc = LopdfDocument::load_mem(&bytes)?;
        Ok(Self { bytes, doc })
    }

    /// Get the number of pages in the PDF
    pub fn page_count(&self) -> usize {
        self.doc.get_pages().len()
    }

    /// Save PDF to a file for manual debugging
    pub fn save_for_debug(&self, name: &str) -> std::io::Result<()> {
        std::fs::write(format!("test_output_{}.pdf", name), &self.bytes)
    }
}

/// Generate a PDF from a JSON template with empty data
pub fn generate_pdf_from_json(template: &Value) -> Result<GeneratedPdf, PipelineError> {
    generate_pdf_from_json_with_data(template, serde_json::json!({}))
}

/// Generate a PDF from a JSON template with provided data
pub fn generate_pdf_from_json_with_data(
    template: &Value,
    data: Value,
) -> Result<GeneratedPdf, PipelineError> {
    let template_str = serde_json::to_string(template)?;
    let pipeline = PipelineBuilder::new()
        .with_template_source(&template_str, "json")?
        .build()?;

    let writer = Cursor::new(Vec::new());
    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(vec![data].into_iter(), writer).await })?;

    let bytes = result.into_inner();
    GeneratedPdf::from_bytes(bytes).map_err(|e| PipelineError::Other(e.to_string()))
}

/// Generate a PDF from an XSLT template with provided data
pub fn generate_pdf_from_xslt(template: &str, data: Value) -> Result<GeneratedPdf, PipelineError> {
    let pipeline = PipelineBuilder::new()
        .with_template_source(template, "xslt")?
        .build()?;

    let writer = Cursor::new(Vec::new());
    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(vec![data].into_iter(), writer).await })?;

    let bytes = result.into_inner();
    GeneratedPdf::from_bytes(bytes).map_err(|e| PipelineError::Other(e.to_string()))
}

/// Generate a PDF in streaming mode (for testing streaming vs composing)
pub fn generate_pdf_streaming(
    template: &Value,
    data: Value,
) -> Result<GeneratedPdf, PipelineError> {
    let template_str = serde_json::to_string(template)?;
    let pipeline = PipelineBuilder::new()
        .with_template_source(&template_str, "json")?
        .with_generation_mode(GenerationMode::ForceStreaming)
        .build()?;

    let writer = Cursor::new(Vec::new());
    let result = tokio::runtime::Runtime::new()?
        .block_on(async { pipeline.generate(vec![data].into_iter(), writer).await })?;

    let bytes = result.into_inner();
    GeneratedPdf::from_bytes(bytes).map_err(|e| PipelineError::Other(e.to_string()))
}
