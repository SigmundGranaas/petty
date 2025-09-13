use crate::error::RenderError;
use crate::layout::{
    LayoutElement, LayoutEngine, PositionedElement,
};
use crate::render::DocumentRenderer;
use crate::stylesheet::{Color, PageLayout, PageSize, Stylesheet, TextAlign};
use handlebars::Handlebars;
use printpdf::font::ParsedFont;
use printpdf::graphics::{LinePoint, PaintMode, Point, Polygon, PolygonRing, WindingOrder};
use printpdf::matrix::TextMatrix;
use printpdf::ops::Op;
use printpdf::text::TextItem;
use printpdf::xobject::{XObject, XObjectTransform};
use printpdf::{
    FontId, Layer, Mm, PdfConformance, PdfDocument, PdfPage, PdfSaveOptions, Pt, Rgb, XObjectId,
};
use serde_json::Value;
use std::collections::HashMap;
use std::io;

pub struct PdfDocumentRenderer {
    document: PdfDocument,
    fonts: HashMap<String, FontId>,
    default_font: FontId,
    page_contexts: Vec<Value>,
    stylesheet: Stylesheet,
    image_xobjects: HashMap<String, (XObjectId, (u32, u32))>,
    layout_engine: LayoutEngine,
}

impl PdfDocumentRenderer {
    pub fn new(layout_engine: LayoutEngine) -> Result<Self, RenderError> {
        let stylesheet = &layout_engine.stylesheet;
        let title = stylesheet.page.title.as_deref().unwrap_or("Document");
        let mut doc = PdfDocument::new(title);
        doc.metadata.info.conformance = PdfConformance::X3_2002_PDF_1_3;

        let font_data = include_bytes!("../../assets/fonts/Helvetica.ttf");
        let mut warnings = Vec::new();
        let font = ParsedFont::from_bytes(font_data, 0, &mut warnings).ok_or_else(|| {
            RenderError::InternalPdfError("Failed to parse built-in font.".to_string())
        })?;
        let default_font = doc.add_font(&font);

        let mut fonts = HashMap::new();
        fonts.insert("Helvetica".to_string(), default_font.clone());

        Ok(PdfDocumentRenderer {
            document: doc,
            fonts,
            default_font,
            page_contexts: Vec::new(),
            stylesheet: stylesheet.clone(),
            image_xobjects: HashMap::new(),
            layout_engine,
        })
    }

    fn get_font(&self, font_family: &str) -> FontId {
        self.fonts
            .get(font_family)
            .cloned()
            .unwrap_or_else(|| self.default_font.clone())
    }

    fn get_page_dimensions_pt(page_layout: &PageLayout) -> (f32, f32) {
        let (w, h) = Self::get_page_dimensions_mm(page_layout);
        (w.into_pt().0, h.into_pt().0)
    }

    fn get_page_dimensions_mm(page_layout: &PageLayout) -> (Mm, Mm) {
        match page_layout.size {
            PageSize::A4 => (Mm(210.0), Mm(297.0)),
            PageSize::Letter => (Mm(215.9), Mm(279.4)),
            PageSize::Legal => (Mm(215.9), Mm(355.6)),
            PageSize::Custom { width, height } => (Pt(width).into(), Pt(height).into()),
        }
    }
}

impl DocumentRenderer for PdfDocumentRenderer {
    fn begin_document(&mut self) -> Result<(), RenderError> {
        Ok(())
    }

    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
    ) -> Result<(), RenderError> {
        self.page_contexts.push(context.clone());
        let page_layout = self.stylesheet.page.clone();
        let (width_mm, height_mm) = Self::get_page_dimensions_mm(&page_layout);

        let mut page_renderer = PageOpsRenderer::new(self, &page_layout);

        for element in elements {
            page_renderer.render_element(&element)?;
        }

        let ops = page_renderer.into_ops();
        let page_num = self.document.pages.len() + 1;
        let mut final_ops = Vec::new();

        let layer_name = format!("Page {} Layer 1", page_num);
        let layer = Layer::new(&*layer_name);
        let layer_id = self.document.add_layer(&layer);
        final_ops.push(Op::BeginLayer { layer_id });
        final_ops.extend(ops);

        let pdf_page = PdfPage::new(width_mm, height_mm, final_ops);
        self.document.pages.push(pdf_page);

        Ok(())
    }

    fn finalize<W: io::Write>(
        mut self,
        mut writer: W,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError> {
        let mut pages = std::mem::take(&mut self.document.pages);
        let total_pages = pages.len();

        for (i, page) in pages.iter_mut().enumerate() {
            let context_data = self.page_contexts.get(i).unwrap_or(&Value::Null);
            let mut context_with_pagination = context_data.clone();
            if let Some(obj) = context_with_pagination.as_object_mut() {
                obj.insert("page_num".to_string(), (i + 1).into());
                obj.insert("total_pages".to_string(), total_pages.into());
            }

            let page_layout = &self.stylesheet.page;

            if let Some(footer_template) = &page_layout.footer_text {
                let style = self.layout_engine.compute_style(page_layout.footer_style.as_deref(), &self.layout_engine.get_default_style());
                let rendered_text = template_engine.render_template(footer_template, &context_with_pagination)
                    .map_err(|e| RenderError::TemplateError(e.to_string()))?;

                let final_text = rendered_text.replace("%p", &(i + 1).to_string()).replace("%t", &total_pages.to_string());
                let (page_width_pt, _) = Self::get_page_dimensions_pt(page_layout);
                let font_id = self.get_font(&style.font_family);
                let color = Rgb::new(style.color.r as f32 / 255.0, style.color.g as f32 / 255.0, style.color.b as f32 / 255.0, None);

                let mut footer_ops = Vec::new();
                footer_ops.push(Op::StartTextSection);
                footer_ops.push(Op::SetFillColor { col: printpdf::color::Color::Rgb(color) });
                footer_ops.push(Op::SetFontSize { size: Pt(style.font_size), font: font_id.clone() });

                let y = page_layout.margins.bottom - style.font_size;
                let mut x = page_layout.margins.left;
                let line_width = self.layout_engine.measure_text_width(&final_text, &style);
                let content_width = page_width_pt - page_layout.margins.left - page_layout.margins.right;

                match style.text_align {
                    TextAlign::Right => x = page_width_pt - page_layout.margins.right - line_width,
                    TextAlign::Center => x = page_layout.margins.left + (content_width - line_width) / 2.0,
                    _ => {}
                }

                let matrix = TextMatrix::Translate(Pt(x), Pt(y));
                footer_ops.push(Op::SetTextMatrix { matrix });
                footer_ops.push(Op::WriteText { items: vec![TextItem::Text(final_text)], font: font_id });
                footer_ops.push(Op::EndTextSection);
                page.ops.extend(footer_ops);
            }
        }

        self.document.pages = pages;
        let mut warnings = Vec::new();
        self.document.save_writer(&mut writer, &PdfSaveOptions::default(), &mut warnings);
        Ok(())
    }
}

/// A helper struct to manage the state of PDF operations for a single page.
struct PageOpsRenderer<'a> {
    doc_renderer: &'a mut PdfDocumentRenderer,
    page_height_pt: f32,
    ops: Vec<Op>,
    is_text_section_open: bool,
    current_font_id: Option<FontId>,
    current_font_size: Option<f32>,
    current_fill_color: Option<printpdf::color::Color>,
}

impl<'a> PageOpsRenderer<'a> {
    fn new(doc_renderer: &'a mut PdfDocumentRenderer, page_layout: &'a PageLayout) -> Self {
        let (_, page_height_pt) = PdfDocumentRenderer::get_page_dimensions_pt(page_layout);
        Self {
            doc_renderer,
            page_height_pt,
            ops: Vec::new(),
            is_text_section_open: false,
            current_font_id: None,
            current_font_size: None,
            current_fill_color: None,
        }
    }

    fn into_ops(mut self) -> Vec<Op> {
        self.close_text_section_if_open();
        self.ops
    }

    fn close_text_section_if_open(&mut self) {
        if self.is_text_section_open {
            self.ops.push(Op::EndTextSection);
            self.is_text_section_open = false;
        }
    }

    fn to_pdf_color(c: &Color) -> printpdf::color::Color {
        printpdf::color::Color::Rgb(Rgb::new(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, None))
    }

    // Add a helper for drawing debug rectangles
    fn draw_debug_rectangle(&mut self, x: f32, y: f32, width: f32, height: f32, color: Rgb) {
        self.close_text_section_if_open(); // Ensure text section is closed before drawing shapes
        let pdf_y = self.page_height_pt - (y + height); // Convert top-left to PDF bottom-left origin

        let polygon = Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint { p: Point { x: Pt(x), y: Pt(pdf_y) }, bezier: false },
                    LinePoint { p: Point { x: Pt(x + width), y: Pt(pdf_y) }, bezier: false },
                    LinePoint { p: Point { x: Pt(x + width), y: Pt(pdf_y + height) }, bezier: false },
                    LinePoint { p: Point { x: Pt(x), y: Pt(pdf_y + height) }, bezier: false },
                ],
            }],
            mode: PaintMode::FillStroke, // Corrected from FillAndStroke
            winding_order: WindingOrder::EvenOdd,
        };
        self.ops.push(Op::SetFillColor { col: printpdf::color::Color::Rgb(color.clone()) });
        self.ops.push(Op::SetOutlineColor { col: printpdf::color::Color::Rgb(color) }); // Corrected from SetStrokeColor
        self.ops.push(Op::SetOutlineThickness { pt: Pt(0.2) }); // Corrected from SetLineWidth
        self.ops.push(Op::DrawPolygon { polygon });
    }

    fn render_element(&mut self, element: &PositionedElement) -> Result<(), RenderError> {
        // Render debug layout overlay if enabled via environment variable
        if std::env::var("PETTY_DEBUG_LAYOUT").is_ok() {
            let debug_color = match element.element {
                LayoutElement::Text(_) => Rgb::new(1.0, 0.0, 0.0, None), // Red for text (alpha not direct in Rgb)
                LayoutElement::Rectangle(_) => Rgb::new(0.0, 0.0, 1.0, None), // Blue for generic blocks (backgrounds)
                LayoutElement::Image(_) => Rgb::new(0.0, 1.0, 0.0, None), // Green for images
            };
            // Note: drawing debug rectangle before content, so close text section if open.
            self.draw_debug_rectangle(element.x, element.y, element.width, element.height, debug_color);
        }

        // Render element's actual background first, if it's not a self-rendering rectangle.
        // `LayoutElement::Rectangle` is often used *for* a background, so we don't want to draw it twice.
        if element.style.background_color.is_some() && !matches!(element.element, LayoutElement::Rectangle(_)) {
            self.render_rectangle(element)?;
        }

        match &element.element {
            LayoutElement::Text(text) => self.render_text(text, element)?,
            LayoutElement::Rectangle(_) => {
                // If this is specifically a LayoutElement::Rectangle, then render it.
                // It's not a background *of another element*, but the element itself.
                self.render_rectangle(element)?;
            }
            LayoutElement::Image(image) => self.render_image(image, element)?,
        }
        Ok(())
    }

    fn render_image(&mut self, image_el: &crate::layout::ImageElement, positioned: &PositionedElement) -> Result<(), RenderError> {
        self.close_text_section_if_open();
        let (xobj_id, (img_w, img_h)) = if let Some(cached) = self.doc_renderer.image_xobjects.get(&image_el.src) {
            (cached.0.clone(), cached.1)
        } else {
            let mut warnings = Vec::new();
            let raw_image = printpdf::image::RawImage::decode_from_bytes(&image_el.image_data, &mut warnings)
                .map_err(|e| RenderError::InternalPdfError(format!("Failed to decode image data for {}: {}", image_el.src, e)))?;
            let dims = (raw_image.width as u32, raw_image.height as u32);
            let xobj_id = XObjectId::new();
            self.doc_renderer.document.resources.xobjects.map.insert(xobj_id.clone(), XObject::Image(raw_image));
            self.doc_renderer.image_xobjects.insert(image_el.src.clone(), (xobj_id.clone(), dims));
            (xobj_id, dims)
        };
        let y = self.page_height_pt - (positioned.y + positioned.height);
        let transform = XObjectTransform {
            translate_x: Some(Pt(positioned.x)),
            translate_y: Some(Pt(y)),
            scale_x: Some(positioned.width / (img_w as f32)),
            scale_y: Some(positioned.height / (img_h as f32)),
            rotate: None, dpi: Some(72.0),
        };
        self.ops.push(Op::UseXobject { id: xobj_id, transform });
        Ok(())
    }

    fn render_rectangle(&mut self, positioned: &PositionedElement) -> Result<(), RenderError> {
        self.close_text_section_if_open();
        if let Some(bg_color) = &positioned.style.background_color {
            let y = self.page_height_pt - (positioned.y + positioned.height);
            let polygon = Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint { p: Point { x: Pt(positioned.x), y: Pt(y) }, bezier: false },
                        LinePoint { p: Point { x: Pt(positioned.x + positioned.width), y: Pt(y) }, bezier: false },
                        LinePoint { p: Point { x: Pt(positioned.x + positioned.width), y: Pt(y + positioned.height) }, bezier: false },
                        LinePoint { p: Point { x: Pt(positioned.x), y: Pt(y + positioned.height) }, bezier: false },
                    ],
                }],
                mode: PaintMode::Fill, winding_order: WindingOrder::EvenOdd,
            };
            self.ops.push(Op::SetFillColor { col: Self::to_pdf_color(bg_color) });
            self.ops.push(Op::DrawPolygon { polygon });
        }
        Ok(())
    }

    fn render_text(&mut self, text: &crate::layout::TextElement, positioned: &PositionedElement) -> Result<(), RenderError> {
        if text.content.is_empty() { return Ok(()); }
        let style = &positioned.style;
        let font_id = self.doc_renderer.get_font(&style.font_family);
        let fill_color = Self::to_pdf_color(&style.color);

        if !self.is_text_section_open { self.ops.push(Op::StartTextSection); self.is_text_section_open = true; }
        if self.current_fill_color.as_ref() != Some(&fill_color) { self.ops.push(Op::SetFillColor { col: fill_color.clone() }); self.current_fill_color = Some(fill_color); }
        if self.current_font_id.as_ref() != Some(&font_id) || self.current_font_size != Some(style.font_size) {
            self.ops.push(Op::SetFontSize { size: Pt(style.font_size), font: font_id.clone() });
            self.current_font_id = Some(font_id.clone()); self.current_font_size = Some(style.font_size);
        }

        // The layout engine now provides perfectly positioned lines. We just need to draw them.
        // The baseline is typically slightly below the top 'y' coordinate.
        let baseline_y = positioned.y + style.font_size * 0.8;
        let pdf_y = self.page_height_pt - baseline_y;

        let matrix = TextMatrix::Translate(Pt(positioned.x), Pt(pdf_y));
        self.ops.push(Op::SetTextMatrix { matrix });
        self.ops.push(Op::WriteText { items: vec![TextItem::Text(text.content.clone())], font: font_id.clone() });

        Ok(())
    }
}