use crate::error::RenderError;
use lopdf::ObjectId;
use petty_idf::SharedData;
use petty_layout::PositionedElement;
use std::any::Any;
use std::collections::HashMap;
use std::io::{Seek, Write};

/// A trait for document renderers, abstracting the PDF-writing primitives.
pub trait DocumentRenderer<W: Write + Seek + Send> {
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError>;

    fn add_resources(&mut self, resources: &HashMap<String, SharedData>)
    -> Result<(), RenderError>;

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
