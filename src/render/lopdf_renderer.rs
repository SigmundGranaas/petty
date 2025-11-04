// src/render/lopdf_renderer.rs
use super::renderer::{DocumentRenderer, RenderError};
use super::streaming_writer::StreamingPdfWriter;
use crate::core::idf::SharedData;
use crate::core::layout::{LayoutEngine, PositionedElement};
use crate::core::style::stylesheet::Stylesheet;
use crate::render::lopdf_helpers;
use lopdf::{dictionary, Dictionary, Object, ObjectId};
use std::any::Any;
use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::Arc;

/// A stateless PDF renderer using the `lopdf` library.
///
/// This renderer acts as a "toolkit" driven by a higher-level strategy. It is responsible
/// for low-level PDF object creation but holds no state about the document's overall
/// structure (like the number of pages or the location of anchors).
pub struct LopdfRenderer<W: Write + Send> {
    final_writer: Option<W>,
    writer: Option<StreamingPdfWriter<Cursor<Vec<u8>>>>,
    stylesheet: Arc<Stylesheet>,
    layout_engine: LayoutEngine,
    font_map: HashMap<String, String>,
    outline_root_id: Option<ObjectId>,
}

impl<W: Write + Send> LopdfRenderer<W> {
    pub fn new(layout_engine: LayoutEngine, stylesheet: Arc<Stylesheet>) -> Result<Self, RenderError> {
        let mut font_map = HashMap::new();
        for (i, face) in layout_engine.font_manager.db().faces().enumerate() {
            font_map.insert(face.post_script_name.clone(), format!("F{}", i + 1));
        }

        Ok(Self {
            final_writer: None,
            writer: None,
            stylesheet,
            layout_engine,
            font_map,
            outline_root_id: None,
        })
    }

    /// Provides mutable access to the underlying `StreamingPdfWriter`, allowing
    /// helper functions to buffer objects directly.
    pub fn writer_mut(&mut self) -> Option<&mut StreamingPdfWriter<Cursor<Vec<u8>>>> {
        self.writer.as_mut()
    }

    /// Writes a Page object dictionary with a specific ID.
    pub fn write_page_object_at_id(
        &mut self,
        page_id: ObjectId,
        content_stream_ids: Vec<ObjectId>,
        annotations: Vec<ObjectId>,
        page_width: f32,
        page_height: f32,
    ) -> Result<(), RenderError> {
        let writer = self.writer.as_mut().ok_or_else(|| RenderError::Other("Document not started".into()))?;

        let mut page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => writer.pages_id,
            "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
            "Contents" => Object::Array(content_stream_ids.into_iter().map(Object::Reference).collect()),
            "Resources" => writer.resources_id,
        };
        if !annotations.is_empty() {
            page_dict.set("Annots", Object::Array(annotations.into_iter().map(Object::Reference).collect()));
        }

        writer.buffer_object_at_id(page_id, page_dict.into());
        Ok(())
    }
}

impl<W: Write + Send + 'static> DocumentRenderer<W> for LopdfRenderer<W> {
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError> {
        self.final_writer = Some(writer);
        let buffer = Cursor::new(Vec::new());

        let mut font_dict = Dictionary::new();
        for face in self.layout_engine.font_manager.db().faces() {
            if let Some(internal_name) = self.font_map.get(&face.post_script_name) {
                let single_font_dict = dictionary! {
                    "Type" => "Font", "Subtype" => "Type1", "BaseFont" => face.post_script_name.clone(), "Encoding" => "WinAnsiEncoding",
                };
                font_dict.set(internal_name.as_bytes(), Object::Dictionary(single_font_dict));
            }
        }

        self.writer = Some(StreamingPdfWriter::new(buffer, "1.7", font_dict)?);
        Ok(())
    }

    fn add_resources(&mut self, _resources: &HashMap<String, SharedData>) -> Result<(), RenderError> {
        // TODO: Implement image resource handling for lopdf
        Ok(())
    }

    fn render_page_content(
        &mut self,
        elements: Vec<PositionedElement>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError> {
        let writer = self.writer.as_mut().ok_or_else(|| RenderError::Other("Document not started".into()))?;
        let content = lopdf_helpers::render_elements_to_content(
            elements,
            &self.layout_engine,
            &self.stylesheet,
            page_width,
            page_height,
        )?;
        let content_id = writer.buffer_content_stream(content);
        Ok(content_id)
    }

    fn write_page_object(
        &mut self,
        content_stream_ids: Vec<ObjectId>,
        annotations: Vec<ObjectId>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError> {
        let writer = self.writer.as_mut().ok_or_else(|| RenderError::Other("Document not started".into()))?;

        let mut page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => writer.pages_id,
            "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
            "Contents" => Object::Array(content_stream_ids.into_iter().map(Object::Reference).collect()),
            "Resources" => writer.resources_id,
        };
        if !annotations.is_empty() {
            page_dict.set("Annots", Object::Array(annotations.into_iter().map(Object::Reference).collect()));
        }

        let page_id = writer.buffer_object(page_dict.into());
        Ok(page_id)
    }

    fn set_outline_root(&mut self, outline_root_id: ObjectId) {
        self.outline_root_id = Some(outline_root_id);
    }

    fn finish(mut self: Box<Self>, page_ids: Vec<ObjectId>) -> Result<(), RenderError> {
        if let Some(mut writer) = self.writer.take() {
            writer.set_page_ids(page_ids);
            writer.set_outline_root_id(self.outline_root_id);
            let buffer = writer.finish()?;
            if let Some(final_writer) = self.final_writer.as_mut() {
                final_writer.write_all(buffer.get_ref())?;
            }
        }
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}