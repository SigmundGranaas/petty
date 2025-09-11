// src/render/mod.rs
use crate::error::RenderError;
use crate::layout::LayoutEngine;
use crate::layout::PositionedElement;
use crate::stylesheet::PageLayout;
use handlebars::Handlebars;
use serde_json::Value;
use std::io;

pub mod pdf;

/// A trait defining the API for a document renderer.
/// The layout engine uses this trait to draw elements, making it possible
/// to swap rendering backends (e.g., PDF, HTML, etc.).
pub trait DocumentRenderer<'a> {
    fn begin_document(&mut self) -> Result<(), RenderError>;
    fn begin_page(&mut self, page_layout: &PageLayout) -> Result<(), RenderError>;
    fn end_page(&mut self);
    fn start_new_logical_page(&mut self, context: &'a Value);
    fn render_element(
        &mut self,
        element: &PositionedElement,
        layout_engine: &LayoutEngine,
    ) -> Result<(), RenderError>;

    // --- NEW: Hyperlink Handling ---
    fn start_hyperlink(&mut self, href: &str);
    fn end_hyperlink(&mut self);

    fn finalize<W: io::Write>(
        self,
        writer: W,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError>;
}