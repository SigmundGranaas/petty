use crate::core::idf::SharedData;
use crate::core::layout::PositionedElement;
use crate::pipeline::worker::LaidOutSequence;
use handlebars::Handlebars;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF generation error: {0}")]
    Pdf(String),
    #[error("Internal PDF library error: {0}")]
    PdfLibError(String),
    #[error("Internal PDF error: {0}")]
    InternalPdfError(String),
    #[error("Template rendering error: {0}")]
    Template(#[from] handlebars::RenderError),
    #[error("Other rendering error: {0}")]
    Other(String),
}

impl From<lopdf::Error> for RenderError {
    fn from(err: lopdf::Error) -> Self {
        RenderError::Pdf(err.to_string())
    }
}

/// The final, resolved location of an anchor in the document.
#[derive(Debug, Clone)]
pub struct ResolvedAnchor {
    /// The final, 1-based global page number.
    pub global_page_index: usize,
    /// The Y position on the page, in points.
    pub y_pos: f32,
}

/// A trait for document renderers, abstracting the final output format (e.g., PDF).
pub trait DocumentRenderer<W: Write + Send> {
    /// Initializes the document and sets up the writer.
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError>;

    /// Adds binary resources (like images) to the document.
    fn add_resources(&mut self, resources: &HashMap<String, SharedData>) -> Result<(), RenderError>;

    /// Renders a single page of elements.
    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError>;

    /// Performs the final "fix-up" pass after all pages have been streamed.
    /// This is used to write objects that depend on the final location of all content,
    /// such as the table of contents and cross-references.
    fn finalize(
        &mut self,
        resolved_anchors: &HashMap<String, ResolvedAnchor>,
        sequences: &[LaidOutSequence],
    ) -> Result<(), RenderError>;

    /// Writes the final document trailer and closes the stream.
    fn finish(self: Box<Self>) -> Result<(), RenderError>;
}