use super::drawing;
use crate::error::RenderError;
use crate::layout::{LayoutEngine, PositionedElement};
use crate::render::DocumentRenderer;
use crate::stylesheet::{PageLayout, PageSize, Stylesheet, TextAlign};
use handlebars::Handlebars;
use printpdf::font::ParsedFont;
use printpdf::image::RawImage;
use printpdf::ops::Op;
use printpdf::xobject::XObject;
use printpdf::{FontId, Layer, Mm, PdfConformance, PdfDocument, PdfPage, PdfSaveOptions, Pt, Rgb, TextItem, TextMatrix, XObjectId};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;

/// Manages the state of the entire PDF document, including pages, fonts, and global resources.
pub struct PdfDocumentRenderer {
    document: PdfDocument,
    pub(super) fonts: HashMap<String, FontId>,
    pub(super) default_font: FontId,
    stylesheet: Stylesheet,
    pub(super) image_xobjects: HashMap<String, (XObjectId, (u32, u32))>,
    layout_engine: LayoutEngine,
}

impl PdfDocumentRenderer {
    /// Creates a new document renderer.
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
            stylesheet: stylesheet.clone(),
            image_xobjects: HashMap::new(),
            layout_engine,
        })
    }

    /// Decodes image data, adds it as an XObject to the PDF resources, and caches it.
    /// This method provides controlled access to the private `document` field.
    pub(super) fn add_image_xobject(
        &mut self,
        src: &str,
        image_data: &Arc<Vec<u8>>,
    ) -> Result<(XObjectId, (u32, u32)), RenderError> {
        let mut warnings = Vec::new();
        let raw_image = RawImage::decode_from_bytes(image_data, &mut warnings).map_err(|e| {
            RenderError::InternalPdfError(format!(
                "Failed to decode image data for {}: {}",
                src, e
            ))
        })?;
        let dims = (raw_image.width as u32, raw_image.height as u32);
        let xobj_id = XObjectId::new();
        self.document
            .resources
            .xobjects
            .map
            .insert(xobj_id.clone(), XObject::Image(raw_image));
        self.image_xobjects.insert(src.to_string(), (xobj_id.clone(), dims));
        Ok((xobj_id, dims))
    }

    /// Gets the dimensions of the page in points.
    fn get_page_dimensions_pt(page_layout: &PageLayout) -> (f32, f32) {
        let (w, h) = Self::get_page_dimensions_mm(page_layout);
        (w.into_pt().0, h.into_pt().0)
    }

    /// Gets the dimensions of the page in millimeters.
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
        template_engine: &Handlebars,
    ) -> Result<(), RenderError> {
        // FIX: Clone the page layout to avoid a long-lived immutable borrow of `self`.
        let page_layout = self.stylesheet.page.clone();
        let (width_mm, height_mm) = Self::get_page_dimensions_mm(&page_layout);
        let (_, page_height_pt) = Self::get_page_dimensions_pt(&page_layout);
        let page_num = self.document.pages.len() + 1;

        // Use a PageRenderer to generate the operations for the page's main content.
        let page_ops = {
            let mut page_renderer = PageRenderer::new(self, page_height_pt);
            page_renderer.render_elements(elements)?;
            page_renderer.into_ops()
        };

        // Add layer operations and the main content.
        let mut final_ops = Vec::new();
        let layer_name = format!("Page {} Layer 1", page_num);
        let layer = Layer::new(&*layer_name);
        let layer_id = self.document.add_layer(&layer);
        final_ops.push(Op::BeginLayer { layer_id });
        final_ops.extend(page_ops);

        // Render the footer and add its operations.
        if let Some(footer_ops) = self.render_footer(context, &page_layout, page_num, template_engine)? {
            final_ops.extend(footer_ops);
        }

        // Add the completed page to the document.
        let pdf_page = PdfPage::new(width_mm, height_mm, final_ops);
        self.document.pages.push(pdf_page);

        Ok(())
    }

    fn finalize(self: Box<Self>, mut writer: Box<dyn io::Write + Send>) -> Result<(), RenderError> {
        let mut warnings = Vec::new();
        self.document
            .save_writer(&mut writer, &PdfSaveOptions::default(), &mut warnings);
        Ok(())
    }
}

// PERF: A lightweight wrapper struct to pass data to the footer template.
// Using `#[serde(flatten)]` avoids cloning the entire original data context,
// which was a major source of allocations.
#[derive(Serialize)]
struct FooterRenderContext<'a> {
    #[serde(flatten)]
    data: &'a Value,
    page_num: usize,
    total_pages: &'static str,
}

impl PdfDocumentRenderer {
    /// Renders the footer for a given page and returns the PDF operations.
    fn render_footer(
        &self,
        context: &Value,
        page_layout: &PageLayout,
        page_num: usize,
        template_engine: &Handlebars,
    ) -> Result<Option<Vec<Op>>, RenderError> {
        let footer_template = match &page_layout.footer_text {
            Some(text) => text,
            None => return Ok(None),
        };

        // PERF: Use the lightweight wrapper context to avoid a deep clone of `context`.
        let footer_context = FooterRenderContext {
            data: context,
            page_num,
            total_pages: "?", // Total pages is unknown at this stage
        };

        let style = self.layout_engine.compute_style(
            page_layout.footer_style.as_deref(),
            None,
            &self.layout_engine.get_default_style(),
        );

        let rendered_text = template_engine
            .render_template(footer_template, &footer_context)
            .map_err(|e| RenderError::TemplateError(e.to_string()))?;

        // Manual replacement for legacy placeholders for now.
        let final_text = rendered_text
            .replace("%p", &page_num.to_string())
            .replace("%t", "?");

        let (page_width_pt, _) = Self::get_page_dimensions_pt(page_layout);
        let font_id = self.fonts.get(style.font_family.as_str()).unwrap_or(&self.default_font);
        let color = Rgb::new(
            style.color.r as f32 / 255.0,
            style.color.g as f32 / 255.0,
            style.color.b as f32 / 255.0,
            None,
        );

        let mut footer_ops = Vec::new();
        footer_ops.push(Op::StartTextSection);
        footer_ops.push(Op::SetFillColor { col: printpdf::color::Color::Rgb(color) });
        footer_ops.push(Op::SetFontSize { size: Pt(style.font_size), font: font_id.clone() });

        let y = page_layout.margins.bottom - style.font_size;
        let line_width = self.layout_engine.measure_text_width(&final_text, &style);
        let content_width = page_width_pt - page_layout.margins.left - page_layout.margins.right;

        let mut x = page_layout.margins.left;
        match style.text_align {
            TextAlign::Right => x = page_width_pt - page_layout.margins.right - line_width,
            TextAlign::Center => x = page_layout.margins.left + (content_width - line_width) / 2.0,
            _ => {}
        }

        let matrix = TextMatrix::Translate(Pt(x), Pt(y));
        footer_ops.push(Op::SetTextMatrix { matrix });
        footer_ops.push(Op::WriteText { items: vec![TextItem::Text(final_text)], font: font_id.clone() });
        footer_ops.push(Op::EndTextSection);

        Ok(Some(footer_ops))
    }
}

/// Manages the state and generation of PDF operations for a single page's content.
pub(super) struct PageRenderer<'a> {
    pub(super) doc_renderer: &'a mut PdfDocumentRenderer,
    pub(super) page_height_pt: f32,
    pub(super) ops: Vec<Op>,
    pub(super) state: PageRenderState,
}

/// Tracks the current graphics state to avoid redundant PDF operations.
#[derive(Default)]
pub(super) struct PageRenderState {
    pub(super) is_text_section_open: bool,
    // Store the owned, cloneable FontId instead of a reference.
    pub(super) current_font_id: Option<FontId>,
    pub(super) current_font_size: Option<f32>,
    pub(super) current_fill_color: Option<printpdf::color::Color>,
}

impl<'a> PageRenderer<'a> {
    fn new(doc_renderer: &'a mut PdfDocumentRenderer, page_height_pt: f32) -> Self {
        Self {
            doc_renderer,
            page_height_pt,
            ops: Vec::new(),
            state: PageRenderState::default(),
        }
    }

    fn render_elements(&mut self, elements: Vec<PositionedElement>) -> Result<(), RenderError> {
        for element in elements {
            drawing::draw_element(self, &element)?;
        }
        Ok(())
    }

    /// Consumes the renderer and returns the finalized PDF operations for the page.
    fn into_ops(mut self) -> Vec<Op> {
        if self.state.is_text_section_open {
            self.ops.push(Op::EndTextSection);
        }
        self.ops
    }
}