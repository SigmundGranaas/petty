// src/render/renderer.rs
use crate::core::idf::SharedData;
use crate::core::layout::PositionedElement;
use crate::pipeline::worker::TocEntry;
use lopdf::ObjectId;
use std::collections::HashMap;
use std::io::{Seek, Write};
use thiserror::Error;
use std::any::Any;

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

/// The location and target of a hyperlink, collected during the first pass.
#[derive(Debug, Clone)]
pub struct HyperlinkLocation {
    /// The 1-based global page index where the link appears.
    pub global_page_index: usize,
    /// The rectangular area of the link on the page, in points. [x1, y1, x2, y2]
    pub rect: [f32; 4],
    /// The anchor ID this link points to (e.g., "section-1").
    pub target_id: String,
}

/// Result of the analysis pass, containing all forward-reference data.
#[derive(Debug, Clone, Default)]
pub struct Pass1Result {
    pub resolved_anchors: HashMap<String, ResolvedAnchor>,
    pub toc_entries: Vec<TocEntry>,
    pub total_pages: usize,
    pub hyperlink_locations: Vec<HyperlinkLocation>,
}

/// A trait for document renderers, abstracting the PDF-writing primitives.
/// This trait is designed to be driven by a higher-level "strategy" which
/// manages document-level state and orchestration.
pub trait DocumentRenderer<W: Write + Seek + Send> {
    /// Initializes the document and sets up the underlying writer. This should
    /// also prepare any document-wide resources like fonts.
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError>;

    /// Adds binary resources (like images) to the document. This is not yet
    /// implemented for the lopdf backend.
    fn add_resources(&mut self, resources: &HashMap<String, SharedData>) -> Result<(), RenderError>;

    /// Renders the content stream for a single page and returns its ID.
    fn render_page_content(
        &mut self,
        elements: Vec<PositionedElement>,
        font_map: &HashMap<String, String>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError>;

    /// Writes a Page object dictionary, linking to content stream(s) and annotations.
    fn write_page_object(
        &mut self,
        content_stream_ids: Vec<ObjectId>,
        annotations: Vec<ObjectId>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError>;

    /// Informs the renderer of the root outline object ID, which will be linked
    /// into the document catalog during `finish`.
    fn set_outline_root(&mut self, outline_root_id: ObjectId);

    /// Writes the final document structures (like the page tree) and trailer,
    /// and returns the underlying writer.
    fn finish(self: Box<Self>, page_ids: Vec<ObjectId>) -> Result<W, RenderError>;

    // Helper for downcasting, since the orchestrator needs to access the concrete type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}