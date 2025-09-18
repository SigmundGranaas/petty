// src/render/mod.rs
use crate::error::RenderError;
use crate::layout::PositionedElement;
use handlebars::Handlebars;
use serde_json::Value;
use std::io;

pub mod pdf;

/// A trait defining the API for a document renderer.
/// The pipeline uses this trait to draw pages of elements, making it possible
/// to swap rendering backends (e.g., PDF, HTML, etc.).
pub trait DocumentRenderer {
    /// Prepares the renderer for a new document.
    fn begin_document(&mut self) -> Result<(), RenderError>;

    /// Renders a complete page of positioned elements. The context of the `sequence`
    /// that this page belongs to is provided for rendering page-specific metadata
    /// like headers or footers.
    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError>;

    /// Finalizes the document and writes it to the provided output stream.
    /// This is where tasks like adding page numbers to footers are performed.
    fn finalize<W: io::Write>(
        self,
        writer: W,
    ) -> Result<(), RenderError>;
}