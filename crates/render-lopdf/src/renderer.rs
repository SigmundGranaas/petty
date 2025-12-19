// src/render/lopdf_renderer.rs
use petty_render_core::{DocumentRenderer, RenderError};
use crate::writer::StreamingPdfWriter;
use petty_idf::SharedData;
use petty_layout::{LayoutEngine, PositionedElement};
use petty_style::stylesheet::Stylesheet;
use crate::helpers;
use lopdf::{dictionary, Dictionary, Object, ObjectId};
use std::any::Any;
use std::collections::HashMap;
use std::io::{Cursor, Seek, Write};
use std::sync::Arc;

/// A PDF renderer using the `lopdf` library, capable of both streaming and buffering.
pub struct LopdfRenderer<W: Write + Seek + Send> {
    pub(crate) writer: Option<StreamingPdfWriter<W>>,
    pub stylesheet: Arc<Stylesheet>,
    pub layout_engine: LayoutEngine,
    font_map: HashMap<String, String>,
    outline_root_id: Option<ObjectId>,
}

impl<W: Write + Seek + Send> LopdfRenderer<W> {
    pub fn new(
        layout_engine: LayoutEngine,
        stylesheet: Arc<Stylesheet>,
    ) -> Result<Self, RenderError> {
        let mut font_map = HashMap::new();

        // Use registered_fonts() to get all fonts from both fontdb and FontProvider
        for (i, font_info) in layout_engine.registered_fonts().iter().enumerate() {
            font_map.insert(font_info.postscript_name.clone(), format!("F{}", i + 1));
        }

        Ok(Self {
            writer: None,
            stylesheet,
            layout_engine,
            font_map,
            outline_root_id: None,
        })
    }

    #[allow(dead_code)]
    pub fn writer_mut(&mut self) -> Option<&mut StreamingPdfWriter<W>> {
        self.writer.as_mut()
    }

    #[allow(dead_code)]
    pub fn write_page_object_at_id(
        &mut self,
        page_id: ObjectId,
        content_stream_ids: Vec<ObjectId>,
        annotations: Vec<ObjectId>,
        page_width: f32,
        page_height: f32,
    ) -> Result<(), RenderError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| RenderError::Other("Document not started".into()))?;

        let mut page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => writer.pages_id,
            "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
            "Contents" => Object::Array(content_stream_ids.into_iter().map(Object::Reference).collect()),
            "Resources" => writer.resources_id,
        };
        if !annotations.is_empty() {
            page_dict.set(
                "Annots",
                Object::Array(
                    annotations
                        .into_iter()
                        .map(Object::Reference)
                        .collect(),
                ),
            );
        }

        writer.buffer_object_at_id(page_id, page_dict.into());
        Ok(())
    }
}

impl LopdfRenderer<Cursor<Vec<u8>>> {
    /// Convenience method for in-memory completion used by ComposingRenderer
    pub fn finish_into_buffer(mut self, page_ids: Vec<ObjectId>) -> Result<Vec<u8>, RenderError> {
        if let Some(mut writer) = self.writer.take() {
            writer.set_page_ids(page_ids);
            writer.set_outline_root_id(self.outline_root_id);
            let cursor = writer.finish()?;
            Ok(cursor.into_inner())
        } else {
            Err(RenderError::Other(
                "Document not started or already finished".into(),
            ))
        }
    }
}

impl<W: Write + Seek + Send + 'static> DocumentRenderer<W> for LopdfRenderer<W> {
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError> {
        let mut font_dict = Dictionary::new();

        // Use registered_fonts() to get all fonts from both fontdb and FontProvider
        for font_info in self.layout_engine.registered_fonts() {
            if let Some(internal_name) = self.font_map.get(&font_info.postscript_name) {
                let single_font_dict = dictionary! {
                    "Type" => "Font", "Subtype" => "Type1", "BaseFont" => font_info.postscript_name.clone(), "Encoding" => "WinAnsiEncoding",
                };
                font_dict.set(
                    internal_name.as_bytes(),
                    Object::Dictionary(single_font_dict),
                );
            }
        }

        self.writer = Some(StreamingPdfWriter::new(writer, "1.7", font_dict)?);
        Ok(())
    }

    fn add_resources(
        &mut self,
        _resources: &HashMap<String, SharedData>,
    ) -> Result<(), RenderError> {
        Ok(())
    }

    fn render_page_content(
        &mut self,
        elements: Vec<PositionedElement>,
        font_map: &HashMap<String, String>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| RenderError::Other("Document not started".into()))?;
        let content = helpers::render_elements_to_content(
            elements,
            font_map,
            page_width,
            page_height,
        )?;
        // Use write_content_stream to stream immediately
        let content_id = writer.write_content_stream(content)?;
        Ok(content_id)
    }

    fn write_page_object(
        &mut self,
        content_stream_ids: Vec<ObjectId>,
        annotations: Vec<ObjectId>,
        page_width: f32,
        page_height: f32,
    ) -> Result<ObjectId, RenderError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| RenderError::Other("Document not started".into()))?;

        let mut page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => writer.pages_id,
            "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
            "Contents" => Object::Array(content_stream_ids.into_iter().map(Object::Reference).collect()),
            "Resources" => writer.resources_id,
        };
        if !annotations.is_empty() {
            page_dict.set(
                "Annots",
                Object::Array(
                    annotations
                        .into_iter()
                        .map(Object::Reference)
                        .collect(),
                ),
            );
        }

        let page_id = writer.write_object(page_dict.into())?;
        Ok(page_id)
    }

    fn set_outline_root(&mut self, outline_root_id: ObjectId) {
        self.outline_root_id = Some(outline_root_id);
    }

    fn finish(self: Box<Self>, page_ids: Vec<ObjectId>) -> Result<W, RenderError> {
        let mut renderer = *self;
        if let Some(mut internal_writer) = renderer.writer.take() {
            internal_writer.set_page_ids(page_ids);
            internal_writer.set_outline_root_id(renderer.outline_root_id);
            let writer = internal_writer.finish()?;
            Ok(writer)
        } else {
            Err(RenderError::Other(
                "Document was never started with begin_document".into(),
            ))
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}