// src/render/renderer.rs
use crate::core::idf::SharedData;
use crate::core::layout::PositionedElement;
use crate::types::{ApiIndexEntry, TocEntry};
use lopdf::ObjectId;
use std::any::Any;
use std::collections::HashMap;
use std::io::{Seek, Write};
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

impl From<&str> for RenderError {
    fn from(s: &str) -> Self {
        RenderError::Other(s.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedAnchor {
    pub global_page_index: usize,
    pub y_pos: f32,
}

#[derive(Debug, Clone)]
pub struct HyperlinkLocation {
    pub global_page_index: usize,
    pub rect: [f32; 4],
    pub target_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct Pass1Result {
    pub resolved_anchors: HashMap<String, ResolvedAnchor>,
    pub toc_entries: Vec<TocEntry>,
    pub total_pages: usize,
    pub hyperlink_locations: Vec<HyperlinkLocation>,
    pub index_entries: Vec<ApiIndexEntry>,
}

/// A trait for document renderers, abstracting the PDF-writing primitives.
pub trait DocumentRenderer<W: Write + Seek + Send> {
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError>;

    fn add_resources(&mut self, resources: &HashMap<String, SharedData>) -> Result<(), RenderError>;

    fn render_page_content(
        &mut self,
        elements: Vec<PositionedElement>,
        font_map: &HashMap<String, String>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError>;

    fn write_page_object(
        &mut self,
        content_stream_ids: Vec<ObjectId>,
        annotations: Vec<ObjectId>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError>;

    #[allow(dead_code)]
    fn set_outline_root(&mut self, outline_root_id: ObjectId);

    fn finish(self: Box<Self>, page_ids: Vec<ObjectId>) -> Result<W, RenderError>;

    #[allow(dead_code)]
    fn as_any_mut(&mut self) -> &mut dyn Any;
}