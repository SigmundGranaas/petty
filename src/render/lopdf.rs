// src/render/lopdf.rs

use super::DocumentRenderer;
use crate::error::RenderError;
use crate::layout::style::ComputedStyle;
use crate::layout::{ImageElement, LayoutElement, LayoutEngine, PositionedElement, TextElement};
use crate::stylesheet::{Color, PageLayout, PageSize, Stylesheet, TextAlign};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use handlebars::Handlebars;
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use serde::Serialize;
use serde_json::Value;
use std::io;
use std::io::Write;
use std::sync::Arc;

/// An in-memory PDF renderer using the `lopdf` library.
/// It builds the document's object graph and then writes it to the output stream.
pub struct LopdfDocumentRenderer {
    document: Document,
    pages_id: ObjectId,
    page_ids: Vec<ObjectId>,
    resources_id: ObjectId,
    stylesheet: Stylesheet,
    layout_engine: LayoutEngine,
}

impl LopdfDocumentRenderer {
    pub fn new(layout_engine: LayoutEngine) -> Result<Self, RenderError> {
        let mut document = Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let resources_id = document.new_object_id();
        let stylesheet = layout_engine.stylesheet.clone();

        Ok(Self {
            document,
            pages_id,
            page_ids: Vec::new(),
            resources_id,
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

impl DocumentRenderer for LopdfDocumentRenderer {
    fn begin_document(&mut self) -> Result<(), RenderError> {
        let font_dict_obj = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        };
        let font_id = self.document.add_object(font_dict_obj);

        // This is the central resources dictionary for the entire document.
        let resources_dict = dictionary! {
            "Font" => dictionary! {
                "F1" => font_id, // All text will refer to this font as /F1
            },
        };
        self.document
            .objects
            .insert(self.resources_id, Object::Dictionary(resources_dict));

        // The root Pages object. It does NOT need a resources link itself if children have it.
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => 0,
        };
        self.document
            .objects
            .insert(self.pages_id, Object::Dictionary(pages_dict));

        let catalog_dict = dictionary! { "Type" => "Catalog", "Pages" => self.pages_id };
        let catalog_id = self.document.add_object(catalog_dict);
        self.document.trailer.set("Root", catalog_id);

        Ok(())
    }

    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError> {
        let page_layout = self.stylesheet.page.clone();
        let (page_width, page_height) = Self::get_page_dimensions_pt(&page_layout);
        let page_num = self.page_ids.len() + 1;

        let mut page_ctx = PageContext::new(self, page_height);

        for element in elements {
            page_ctx.draw_element(&element)?;
        }

        if let Some(footer_template) = &page_layout.footer_text {
            page_ctx.draw_footer(context, template_engine, footer_template, &page_layout, page_num)?;
        }

        let content = page_ctx.finish();

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&content.encode()?)?;
        let compressed_content = encoder.finish()?;
        let content_stream = Stream::new(dictionary! {"Filter" => "FlateDecode"}, compressed_content);
        let content_id = self.document.add_object(content_stream);

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => self.pages_id,
            "MediaBox" => vec![0.into(), 0.into(), page_width.into(), page_height.into()],
            "Contents" => content_id,
            "Resources" => self.resources_id,
        };
        let page_id = self.document.add_object(page_dict);
        self.page_ids.push(page_id);

        Ok(())
    }

    fn finalize(mut self: Box<Self>, mut writer: Box<dyn io::Write + Send>) -> Result<(), RenderError> {
        if let Some(Object::Dictionary(pages_dict)) = self.document.objects.get_mut(&self.pages_id) {
            let kids: Vec<Object> = self.page_ids.iter().map(|id| Object::from(*id)).collect();
            pages_dict.set("Kids", kids);
            pages_dict.set("Count", self.page_ids.len() as i32);
        }
        self.document.save_to(&mut writer)?;
        Ok(())
    }
}

struct PageContext<'a> {
    renderer: &'a mut LopdfDocumentRenderer,
    page_height: f32,
    content: Content,
    state: PageRenderState,
}

#[derive(Default, Clone)]
struct PageRenderState {
    font_name: String,
    font_size: f32,
    fill_color: Color,
}

impl<'a> PageContext<'a> {
    fn new(renderer: &'a mut LopdfDocumentRenderer, page_height: f32) -> Self {
        Self {
            renderer,
            page_height,
            content: Content { operations: vec![] },
            state: PageRenderState::default(),
        }
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
            self.content.operations.push(Operation::new("rg", vec![(bg.r as f32 / 255.0).into(), (bg.g as f32 / 255.0).into(), (bg.b as f32 / 255.0).into()]));
            self.content.operations.push(Operation::new("re", vec![x.into(), y.into(), el.width.into(), el.height.into()]));
            self.content.operations.push(Operation::new("f", vec![]));
        }
        if let Some(border) = &style.border_bottom {
            self.content.operations.push(Operation::new("w", vec![border.width.into()]));
            self.content.operations.push(Operation::new("RG", vec![(border.color.r as f32 / 255.0).into(), (border.color.g as f32 / 255.0).into(), (border.color.b as f32 / 255.0).into()]));
            let line_y = self.page_height - (el.y + el.height);
            self.content.operations.push(Operation::new("m", vec![el.x.into(), line_y.into()]));
            self.content.operations.push(Operation::new("l", vec![(el.x + el.width).into(), line_y.into()]));
            self.content.operations.push(Operation::new("S", vec![]));
        }
        Ok(())
    }

    fn set_font(&mut self, style: &Arc<ComputedStyle>) {
        let font_name = "F1";
        if self.state.font_name != font_name || self.state.font_size != style.font_size {
            self.content.operations.push(Operation::new("Tf", vec![font_name.into(), style.font_size.into()]));
            self.state.font_name = font_name.to_string();
            self.state.font_size = style.font_size;
        }
    }

    fn set_fill_color(&mut self, color: &Color) {
        if self.state.fill_color != *color {
            self.content.operations.push(Operation::new("rg", vec![(color.r as f32 / 255.0).into(), (color.g as f32 / 255.0).into(), (color.b as f32 / 255.0).into()]));
            self.state.fill_color = color.clone();
        }
    }

    fn draw_text(&mut self, text: &TextElement, el: &PositionedElement) -> Result<(), RenderError> {
        if text.content.trim().is_empty() { return Ok(()); }
        self.content.operations.push(Operation::new("BT", vec![]));
        self.set_font(&el.style);
        self.set_fill_color(&el.style.color);
        let baseline_y = el.y + el.style.font_size * 0.8;
        let pdf_y = self.page_height - baseline_y;
        self.content.operations.push(Operation::new("Td", vec![el.x.into(), pdf_y.into()]));
        self.content.operations.push(Operation::new("Tj", vec![Object::string_literal(text.content.clone())]));
        self.content.operations.push(Operation::new("ET", vec![]));
        Ok(())
    }

    fn draw_image(&mut self, image: &ImageElement, _el: &PositionedElement) -> Result<(), RenderError> {
        log::warn!("Images are not supported in the lopdf buffering renderer yet: {}", image.src);
        Ok(())
    }

    fn draw_footer(&mut self, context: &Value, template_engine: &Handlebars, footer_template: &str, page_layout: &PageLayout, page_num: usize) -> Result<(), RenderError> {
        let style = self.renderer.layout_engine.compute_style(page_layout.footer_style.as_deref(), None, &self.renderer.layout_engine.get_default_style());
        #[derive(Serialize)] struct FooterCtx<'a> { #[serde(flatten)] data: &'a Value, page_num: usize }
        let ctx = FooterCtx { data: context, page_num };
        let text = template_engine.render_template(footer_template, &ctx)?;
        let (page_width, _) = LopdfDocumentRenderer::get_page_dimensions_pt(page_layout);

        self.content.operations.push(Operation::new("BT", vec![]));
        self.set_font(&style);
        self.set_fill_color(&style.color);

        let line_width = self.renderer.layout_engine.measure_text_width(&text, &style);
        let y = page_layout.margins.bottom - style.font_size;
        let x = match style.text_align {
            TextAlign::Left => page_layout.margins.left,
            TextAlign::Right => page_width - page_layout.margins.right - line_width,
            TextAlign::Center => {
                let content_width = page_width - page_layout.margins.left - page_layout.margins.right;
                page_layout.margins.left + (content_width - line_width) / 2.0
            }
            TextAlign::Justify => page_layout.margins.left,
        };

        self.content.operations.push(Operation::new("Td", vec![x.into(), y.into()]));
        self.content.operations.push(Operation::new("Tj", vec![Object::string_literal(text)]));
        self.content.operations.push(Operation::new("ET", vec![]));
        Ok(())
    }
}

impl From<lopdf::Error> for RenderError {
    fn from(e: lopdf::Error) -> Self { RenderError::PdfLibError(e.to_string()) }
}

impl From<handlebars::RenderError> for RenderError {
    fn from(e: handlebars::RenderError) -> Self { RenderError::TemplateError(e.to_string()) }
}