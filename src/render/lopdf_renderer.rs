use super::renderer::DocumentRenderer;
use super::streaming_writer::internal_writer::write_object;
use super::streaming_writer::StreamingPdfWriter;

use crate::core::style::font::FontWeight;
use crate::render::RenderError;
use handlebars::Handlebars;
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Object, ObjectId, Stream};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use crate::core::idf::SharedData;
use crate::core::layout::{ComputedStyle, ImageElement, LayoutElement, LayoutEngine, PositionedElement, TextElement};
use crate::core::style::color::Color;
use crate::core::style::dimension::PageSize;
use crate::core::style::stylesheet::{PageLayout, Stylesheet};
use crate::core::style::text::TextAlign;

static DEFAULT_LOPDF_FONT_NAME: Lazy<String> = Lazy::new(|| "F1".to_string());


/// A streaming PDF renderer using the `lopdf` library.
/// It writes the document's objects to the output stream as they are generated,
/// minimizing peak memory usage.
pub struct LopdfDocumentRenderer<W: Write + Send> {
    pub writer: Option<StreamingPdfWriter<W>>,
    pub stylesheet: Arc<Stylesheet>,
    pub layout_engine: LayoutEngine,
    // NEW: Map from descriptive name ("Helvetica-Bold") to internal PDF name ("F2")
    font_map: HashMap<String, String>,
}

/// Generates the specific font family name based on style (e.g., "Helvetica-Bold").
/// This logic MUST mirror the logic in `FontManager`.
fn get_styled_font_name(style: &Arc<ComputedStyle>) -> String {
    let family = &style.font_family;
    match style.font_weight {
        FontWeight::Bold | FontWeight::Black => format!("{}-Bold", family),
        _ => family.to_string(),
    }
}

impl<W: Write + Send> LopdfDocumentRenderer<W> {
    pub fn new(layout_engine: LayoutEngine) -> Result<Self, RenderError> {
        let stylesheet = Arc::new(layout_engine.stylesheet.clone());
        let mut font_map = HashMap::new();

        // Build the map of descriptive font names to internal PDF font names (F1, F2, etc.)
        for (i, family_name) in layout_engine.font_manager.font_data.keys().enumerate() {
            font_map.insert(family_name.clone(), format!("F{}", i + 1));
        }

        Ok(Self {
            writer: None,
            stylesheet,
            layout_engine,
            font_map,
        })
    }

    pub(crate) fn get_page_dimensions_pt(page_layout: &PageLayout) -> (f32, f32) {
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
        // Build the font dictionary for the PDF Resources object
        let mut font_dict = Dictionary::new();
        for (family_name, _font_data) in self.layout_engine.font_manager.font_data.iter() {
            if let Some(internal_name) = self.font_map.get(family_name) {
                // For simplicity, we'll assume all are Type1 Helvetica for now.
                // A full implementation would parse the TTF to get the correct subtype.
                let single_font_dict = dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => family_name.clone(),
                };
                // NOTE: A full implementation would require embedding the font program stream.
                // This will work for standard fonts like Helvetica.
                font_dict.set(internal_name.as_bytes(), Object::Dictionary(single_font_dict));
            }
        }

        self.writer = Some(StreamingPdfWriter::new(writer, "1.7", font_dict)?);
        Ok(())
    }

    fn add_resources(&mut self, _resources: &HashMap<String, SharedData>) -> Result<(), RenderError> {
        // Lopdf renderer does not yet support images, so this is a no-op.
        Ok(())
    }

    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError> {
        let writer = self.writer.as_mut().ok_or_else(|| {
            RenderError::Other("begin_document must be called before render_page".into())
        })?;

        let page_layout = self.stylesheet.page.clone();
        let (page_width, page_height) = Self::get_page_dimensions_pt(&page_layout);

        let mut page_ctx = PageContext::new(&self.layout_engine, page_height, &self.font_map);

        for element in elements {
            page_ctx.draw_element(&element)?;
        }

        let page_num = writer.page_ids.len() + 1;
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

// This struct holds the pre-allocated IDs for a single page.
#[derive(Clone)]
pub struct LopdfPageRenderTask {
    pub page_object_id: ObjectId,
    pub content_object_id: ObjectId,
    pub elements: Vec<PositionedElement>,
    pub context: Arc<Value>,
}

/// Renders a single page's elements and footer into raw bytes for its PDF objects.
/// This is a pure function that can be run in parallel.
pub fn render_lopdf_page_to_bytes(
    task: LopdfPageRenderTask,
    page_layout: &PageLayout,
    page_num: usize,
    resources_id: ObjectId,
    parent_pages_id: ObjectId,
    layout_engine: &LayoutEngine,
    template_engine: &Handlebars,
) -> Result<Vec<u8>, RenderError> {
    let mut buffer = Vec::with_capacity(4096); // Start with a 4KB buffer
    let (page_width, page_height) =
        LopdfDocumentRenderer::<&mut Vec<u8>>::get_page_dimensions_pt(page_layout);

    // Create a temporary PageContext to generate the content stream.
    // The writer is a dummy, as we only need the generated operations.
    // In a full implementation, we would need to create a font map here too.
    let font_map = HashMap::new(); // Dummy for now.
    let mut page_ctx = PageContext::new(
        layout_engine, // We only need the layout engine for footer text measurement
        page_height,
        &font_map,
    );

    // Render main elements
    for element in &task.elements {
        page_ctx.draw_element(element)?;
    }

    // Render footer
    if let Some(footer_template) = &page_layout.footer_text {
        page_ctx.draw_footer(
            &task.context,
            template_engine,
            footer_template,
            page_layout,
            page_num,
        )?;
    }

    let content = page_ctx.finish();
    let content_stream = Stream::new(dictionary! {}, content.encode()?);

    // Manually write the objects to our byte buffer using the pre-allocated IDs.
    // This is a simplified version of what lopdf's writer does.
    buffer.extend_from_slice(
        format!(
            "{} {} obj\n",
            task.content_object_id.0, task.content_object_id.1
        )
            .as_bytes(),
    );
    write_object(&mut buffer, &Object::Stream(content_stream))?;
    buffer.extend_from_slice(b"\nendobj\n");

    let page_dict = dictionary! {
        "Type" => "Page",
        "Parent" => parent_pages_id,
        "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
        "Contents" => task.content_object_id,
        "Resources" => resources_id,
    };
    buffer.extend_from_slice(
        format!(
            "{} {} obj\n",
            task.page_object_id.0, task.page_object_id.1
        )
            .as_bytes(),
    );
    write_object(&mut buffer, &Object::Dictionary(page_dict))?;
    buffer.extend_from_slice(b"\nendobj\n");

    Ok(buffer)
}

struct PageContext<'a> {
    layout_engine: &'a LayoutEngine,
    page_height: f32,
    content: Content,
    state: LopdfPageRenderState,
    font_map: &'a HashMap<String, String>,
}

#[derive(Default, Clone)]
struct LopdfPageRenderState {
    font_name: String,
    font_size: f32,
    fill_color: Color,
}

impl<'a> PageContext<'a> {
    fn new(layout_engine: &'a LayoutEngine, page_height: f32, font_map: &'a HashMap<String, String>) -> Self {
        Self {
            layout_engine,
            page_height,
            content: Content { operations: vec![] },
            state: LopdfPageRenderState::default(),
            font_map,
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
        let styled_font_name = get_styled_font_name(style);
        let internal_font_name = match self.font_map.get(&styled_font_name) {
            Some(name) => name,
            None => {
                // Fallback logic
                if styled_font_name != style.font_family.as_str() {
                    log::warn!(
                        "Lopdf: Font style '{}' not found, falling back to base '{}'",
                        styled_font_name, style.font_family
                    );
                }
                // --- FIX: Borrow the static string ---
                self.font_map.get(style.font_family.as_str()).unwrap_or(&DEFAULT_LOPDF_FONT_NAME)
            }
        };

        if self.state.font_name != *internal_font_name || self.state.font_size != style.font_size {
            self.content.operations.push(Operation::new(
                "Tf",
                vec![Object::Name(internal_font_name.as_bytes().to_vec()), style.font_size.into()],
            ));
            self.state.font_name = internal_font_name.to_string();
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
        let style_sets = if let Some(style_name) = page_layout.footer_style.as_deref() {
            self.layout_engine
                .stylesheet
                .styles
                .get(style_name)
                .map(|style_arc| vec![Arc::clone(style_arc)])
                .unwrap_or_default()
        } else {
            vec![]
        };

        let style = self.layout_engine.compute_style(
            &style_sets,
            None,
            &self.layout_engine.get_default_style(),
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
        let (page_width, _) =
            LopdfDocumentRenderer::<&mut Vec<u8>>::get_page_dimensions_pt(page_layout);

        self.content.operations.push(Operation::new("BT", vec![]));
        self.set_font(&style);
        self.set_fill_color(&style.color);

        let line_width = self.layout_engine.measure_text_width(&text, &style);
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