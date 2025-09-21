// src/render/lopdf.rs

use super::renderer::DocumentRenderer;
use super::streaming_writer::StreamingPdfWriter;
use crate::error::RenderError;
use crate::layout::{
    ComputedStyle, ImageElement, LayoutElement, LayoutEngine, PositionedElement, TextElement,
};
use crate::stylesheet::{Color, PageLayout, PageSize, Stylesheet, TextAlign};
use handlebars::Handlebars;
use lopdf::content::{Content, Operation};
use lopdf::{Object, ObjectId};
use serde::Serialize;
use serde_json::Value;
use std::io::Write;
use std::sync::Arc;

/// A streaming PDF renderer using the `lopdf` library.
/// It writes the document's objects to the output stream as they are generated,
/// minimizing peak memory usage.
pub struct LopdfDocumentRenderer<W: Write + Send> {
    writer: Option<StreamingPdfWriter<W>>,
    stylesheet: Arc<Stylesheet>,
    layout_engine: LayoutEngine,
}

impl<W: Write + Send> LopdfDocumentRenderer<W> {
    pub fn new(layout_engine: LayoutEngine) -> Result<Self, RenderError> {
        let stylesheet = Arc::new(layout_engine.stylesheet.clone());
        Ok(Self {
            writer: None,
            stylesheet,
            layout_engine,
        })
    }

    fn get_page_dimensions_pt(page_layout: &PageLayout) -> (f32, f32) {
        match page_layout.size {
            PageSize::A4 => (595.0, 842.0),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Custom { width, height } => (width, height),
        }
    }
}

impl<W: Write + Send> DocumentRenderer<W> for LopdfDocumentRenderer<W> {
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError> {
        self.writer = Some(StreamingPdfWriter::new(writer, "1.7")?);
        Ok(())
    }

    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError> {
        // --- BORROW CHECKER FIX ---
        // Get all the info we need from the writer before creating PageContext, which borrows `self`
        let (resources_id, page_ids_len) = self
            .writer
            .as_ref()
            .map(|w| (w.resources_id, w.page_ids.len()))
            .ok_or_else(|| {
                RenderError::Other("begin_document must be called before render_page".into())
            })?;

        let page_layout = self.stylesheet.page.clone();
        let (page_width, page_height) = Self::get_page_dimensions_pt(&page_layout);

        let mut page_ctx = PageContext::new(self, page_height, resources_id, page_ids_len);

        for element in elements {
            page_ctx.draw_element(&element)?;
        }

        let page_num = page_ctx.page_num_so_far() + 1;
        if let Some(footer_template) = &page_layout.footer_text {
            page_ctx.draw_footer(
                context,
                template_engine,
                footer_template,
                &page_layout,
                page_num,
            )?;
        }

        let content = page_ctx.finish();
        let media_box = [0.0, 0.0, page_width, page_height];

        // Now get the writer mutably, which is safe because PageContext is gone.
        let writer = self.writer.as_mut().unwrap();
        writer.add_page(content, media_box)?;

        Ok(())
    }

    fn finalize(mut self: Box<Self>) -> Result<(), RenderError> {
        if let Some(writer) = self.writer.take() {
            writer.finish()?;
            Ok(())
        } else {
            Err(RenderError::Other(
                "Document was never started with begin_document".into(),
            ))
        }
    }
}

struct PageContext<'a, W: Write + Send> {
    renderer: &'a LopdfDocumentRenderer<W>,
    page_height: f32,
    content: Content,
    state: PageRenderState,
    _resources_id: ObjectId,
    initial_page_count: usize,
}

#[derive(Default, Clone)]
struct PageRenderState {
    font_name: String,
    font_size: f32,
    fill_color: Color,
}

impl<'a, W: Write + Send> PageContext<'a, W> {
    fn new(
        renderer: &'a LopdfDocumentRenderer<W>,
        page_height: f32,
        resources_id: ObjectId,
        initial_page_count: usize,
    ) -> Self {
        Self {
            renderer,
            page_height,
            content: Content { operations: vec![] },
            state: PageRenderState::default(),
            _resources_id: resources_id,
            initial_page_count,
        }
    }

    fn page_num_so_far(&self) -> usize {
        self.initial_page_count
    }

    fn finish(self) -> Content {
        self.content
    }

    fn draw_element(&mut self, el: &PositionedElement) -> Result<(), RenderError> {
        self.draw_background_and_borders(el)?;
        match &el.element {
            LayoutElement::Text(text) => self.draw_text(text, el)?,
            LayoutElement::Image(image) => self.draw_image(image, el)?,
            LayoutElement::Rectangle(_) => {}
        }
        Ok(())
    }

    fn draw_background_and_borders(&mut self, el: &PositionedElement) -> Result<(), RenderError> {
        let style = &el.style;
        let x = el.x;
        let y = self.page_height - (el.y + el.height);
        if let Some(bg) = &style.background_color {
            self.content.operations.push(Operation::new(
                "rg",
                vec![
                    (bg.r as f32 / 255.0).into(),
                    (bg.g as f32 / 255.0).into(),
                    (bg.b as f32 / 255.0).into(),
                ],
            ));
            self.content.operations.push(Operation::new(
                "re",
                vec![x.into(), y.into(), el.width.into(), el.height.into()],
            ));
            self.content.operations.push(Operation::new("f", vec![]));
        }
        if let Some(border) = &style.border_bottom {
            self.content
                .operations
                .push(Operation::new("w", vec![border.width.into()]));
            self.content.operations.push(Operation::new(
                "RG",
                vec![
                    (border.color.r as f32 / 255.0).into(),
                    (border.color.g as f32 / 255.0).into(),
                    (border.color.b as f32 / 255.0).into(),
                ],
            ));
            let line_y = self.page_height - (el.y + el.height);
            self.content
                .operations
                .push(Operation::new("m", vec![el.x.into(), line_y.into()]));
            self.content.operations.push(Operation::new(
                "l",
                vec![(el.x + el.width).into(), line_y.into()],
            ));
            self.content.operations.push(Operation::new("S", vec![]));
        }
        Ok(())
    }

    fn set_font(&mut self, style: &Arc<ComputedStyle>) {
        let font_name = "F1";
        if self.state.font_name != font_name || self.state.font_size != style.font_size {
            self.content.operations.push(Operation::new(
                "Tf",
                vec![font_name.into(), style.font_size.into()],
            ));
            self.state.font_name = font_name.to_string();
            self.state.font_size = style.font_size;
        }
    }

    fn set_fill_color(&mut self, color: &Color) {
        if self.state.fill_color != *color {
            self.content.operations.push(Operation::new(
                "rg",
                vec![
                    (color.r as f32 / 255.0).into(),
                    (color.g as f32 / 255.0).into(),
                    (color.b as f32 / 255.0).into(),
                ],
            ));
            self.state.fill_color = color.clone();
        }
    }

    fn draw_text(&mut self, text: &TextElement, el: &PositionedElement) -> Result<(), RenderError> {
        if text.content.trim().is_empty() {
            return Ok(());
        }
        self.content.operations.push(Operation::new("BT", vec![]));
        self.set_font(&el.style);
        self.set_fill_color(&el.style.color);
        let baseline_y = el.y + el.style.font_size * 0.8;
        let pdf_y = self.page_height - baseline_y;
        self.content
            .operations
            .push(Operation::new("Td", vec![el.x.into(), pdf_y.into()]));
        self.content.operations.push(Operation::new(
            "Tj",
            vec![Object::string_literal(text.content.clone())],
        ));
        self.content.operations.push(Operation::new("ET", vec![]));
        Ok(())
    }

    fn draw_image(
        &mut self,
        image: &ImageElement,
        _el: &PositionedElement,
    ) -> Result<(), RenderError> {
        log::warn!(
            "Images are not supported in the lopdf streaming renderer yet: {}",
            image.src
        );
        Ok(())
    }

    fn draw_footer(
        &mut self,
        context: &Value,
        template_engine: &Handlebars,
        footer_template: &str,
        page_layout: &PageLayout,
        page_num: usize,
    ) -> Result<(), RenderError> {
        let style = self.renderer.layout_engine.compute_style(
            page_layout.footer_style.as_deref(),
            None,
            &self.renderer.layout_engine.get_default_style(),
        );
        #[derive(Serialize)]
        struct FooterCtx<'a> {
            #[serde(flatten)]
            data: &'a Value,
            page_num: usize,
        }
        let ctx = FooterCtx {
            data: context,
            page_num,
        };
        let text = template_engine.render_template(footer_template, &ctx)?;
        let (page_width, _) = LopdfDocumentRenderer::<W>::get_page_dimensions_pt(page_layout);

        self.content.operations.push(Operation::new("BT", vec![]));
        self.set_font(&style);
        self.set_fill_color(&style.color);

        let line_width = self.renderer.layout_engine.measure_text_width(&text, &style);
        let y = page_layout.margins.bottom - style.font_size;
        let x = match style.text_align {
            TextAlign::Left => page_layout.margins.left,
            TextAlign::Right => page_width - page_layout.margins.right - line_width,
            TextAlign::Center => {
                let content_width =
                    page_width - page_layout.margins.left - page_layout.margins.right;
                page_layout.margins.left + (content_width - line_width) / 2.0
            }
            TextAlign::Justify => page_layout.margins.left,
        };

        self.content
            .operations
            .push(Operation::new("Td", vec![x.into(), y.into()]));
        self.content
            .operations
            .push(Operation::new("Tj", vec![Object::string_literal(text)]));
        self.content.operations.push(Operation::new("ET", vec![]));
        Ok(())
    }
}