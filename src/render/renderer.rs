use crate::render::RenderError;
use handlebars::Handlebars;
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use crate::core::idf::SharedData;
use crate::core::layout::PositionedElement;

/// A trait for document renderers that can generate a document page by page.
/// This is designed to support streaming output, where pages are written as they
/// are processed to minimize memory usage.
pub trait DocumentRenderer<W: io::Write + Send> {
    /// Initializes the document and writes the header and any necessary scaffolding
    /// to the provided writer. This must be called before any other rendering methods.
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError>;

    /// Ingests resources for the upcoming pages. For example, a PDF renderer would
    /// use this to create shared XObject resources for images.
    fn add_resources(&mut self, resources: &HashMap<String, SharedData>) -> Result<(), RenderError>;

    /// Renders a single page with the given elements and context.
    /// The rendered page is written directly to the output stream.
    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError>;

    /// Finalizes the document, writing any closing structures like the cross-reference
    /// table and trailer. This consumes the renderer.
    fn finalize(self: Box<Self>) -> Result<(), RenderError>;
}