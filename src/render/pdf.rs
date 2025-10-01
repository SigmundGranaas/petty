use super::{drawing, RenderError};
use crate::render::DocumentRenderer;
use handlebars::Handlebars;
use printpdf::font::ParsedFont;
use printpdf::image::RawImage;
use printpdf::ops::Op;
use printpdf::xobject::XObject;
use printpdf::{
    FontId, Layer, Mm, PdfConformance, PdfDocument, PdfPage, PdfSaveOptions, Pt, Rgb, TextItem,
    TextMatrix, XObjectId,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use crate::core::idf::SharedData;
use crate::core::layout::{ComputedStyle, LayoutEngine, PositionedElement};
use crate::core::style::dimension::{Margins, PageSize};
use crate::core::style::font::FontWeight;
use crate::core::style::stylesheet::{PageLayout, Stylesheet};
use crate::core::style::text::TextAlign;

/// Manages the state of the entire PDF document, including pages, fonts, and global resources.
pub struct PdfDocumentRenderer<W: io::Write + Send> {
    pub(crate) document: PdfDocument,
    pub(crate) fonts: HashMap<String, FontId>,
    pub(crate) default_font: FontId,
    pub(crate) stylesheet: Stylesheet,
    pub(crate) image_xobjects: HashMap<String, (XObjectId, (u32, u32))>,
    pub(crate) layout_engine: LayoutEngine,
    pub(crate) writer: Option<W>,
}

impl<W: io::Write + Send> PdfDocumentRenderer<W> {
    /// Creates a new document renderer.
    pub fn new(layout_engine: LayoutEngine) -> Result<Self, RenderError> {
        let stylesheet = &layout_engine.stylesheet;
        let title = stylesheet.page.title.as_deref().unwrap_or("Document");
        let mut doc = PdfDocument::new(title);
        doc.metadata.info.conformance = PdfConformance::X3_2002_PDF_1_3;

        let mut fonts = HashMap::new();
        let mut default_font_id: Option<FontId> = None;

        for (family_name, font_data) in layout_engine.font_manager.font_data.iter() {
            let mut warnings = Vec::new();
            let font = ParsedFont::from_bytes(font_data, 0, &mut warnings).ok_or_else(|| {
                RenderError::InternalPdfError(format!("Failed to parse font {}", family_name))
            })?;
            let font_id = doc.add_font(&font);
            fonts.insert(family_name.clone(), font_id.clone());
            if family_name.eq_ignore_ascii_case("helvetica") {
                default_font_id = Some(font_id);
            }
        }

        let default_font = default_font_id.or_else(|| fonts.values().next().cloned()).ok_or_else(|| {
            RenderError::InternalPdfError("No fonts were loaded, cannot create PDF.".to_string())
        })?;

        Ok(PdfDocumentRenderer {
            document: doc,
            fonts,
            default_font,
            stylesheet: stylesheet.clone(),
            image_xobjects: HashMap::new(),
            layout_engine,
            writer: None,
        })
    }

    /// Decodes image data, adds it as an XObject to the PDF resources, and caches it.
    /// This method provides controlled access to the private `document` field.
    pub(crate) fn add_image_xobject(
        &mut self,
        src: &str,
        image_data: &SharedData,
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
        self.image_xobjects
            .insert(src.to_string(), (xobj_id.clone(), dims));
        Ok((xobj_id, dims))
    }

    /// Gets the dimensions of the page in points.
    pub(crate) fn get_page_dimensions_pt(page_layout: &PageLayout) -> (f32, f32) {
        let (w, h) = Self::get_page_dimensions_mm(page_layout);
        (w.into_pt().0, h.into_pt().0)
    }

    /// Gets the dimensions of the page in millimeters.
    pub(crate) fn get_page_dimensions_mm(page_layout: &PageLayout) -> (Mm, Mm) {
        match page_layout.size {
            PageSize::A4 => (Mm(210.0), Mm(297.0)),
            PageSize::Letter => (Mm(215.9), Mm(279.4)),
            PageSize::Legal => (Mm(215.9), Mm(355.6)),
            PageSize::Custom { width, height } => (Pt(width).into(), Pt(height).into()),
        }
    }
}

impl<W: io::Write + Send> DocumentRenderer<W> for PdfDocumentRenderer<W> {
    fn begin_document(&mut self, writer: W) -> Result<(), RenderError> {
        self.writer = Some(writer);
        Ok(())
    }

    fn add_resources(
        &mut self,
        resources: &HashMap<String, SharedData>,
    ) -> Result<(), RenderError> {
        for (src, data) in resources {
            if !self.image_xobjects.contains_key(src) {
                self.add_image_xobject(src, data)?;
            }
        }
        Ok(())
    }

    fn render_page(
        &mut self,
        context: &Value,
        elements: Vec<PositionedElement>,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError> {
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
        if let Some(footer_ops) =
            self.render_footer(context, &page_layout, page_num, template_engine)?
        {
            final_ops.extend(footer_ops);
        }

        // Add the completed page to the document.
        let pdf_page = PdfPage::new(width_mm, height_mm, final_ops);
        self.document.pages.push(pdf_page);

        Ok(())
    }

    fn finalize(self: Box<Self>) -> Result<(), RenderError> {
        let mut writer = self.writer.ok_or_else(|| {
            RenderError::Other("Document was never started with begin_document".into())
        })?;
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

impl<W: io::Write + Send> PdfDocumentRenderer<W> {
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

        let rendered_text = template_engine
            .render_template(footer_template, &footer_context)
            .map_err(|e| RenderError::TemplateError(e.to_string()))?;

        // Manual replacement for legacy placeholders for now.
        let final_text = rendered_text;
        let default_margins = Margins::default();
        let margins = page_layout.margins.as_ref().unwrap_or(&default_margins);

        let (page_width_pt, _) = Self::get_page_dimensions_pt(page_layout);
        let font_id = self
            .fonts
            .get(style.font_family.as_str())
            .unwrap_or(&self.default_font);
        let color = Rgb::new(
            style.color.r as f32 / 255.0,
            style.color.g as f32 / 255.0,
            style.color.b as f32 / 255.0,
            None,
        );

        let mut footer_ops = Vec::new();
        footer_ops.push(Op::StartTextSection);
        footer_ops.push(Op::SetFillColor {
            col: printpdf::color::Color::Rgb(color),
        });
        footer_ops.push(Op::SetFontSize {
            size: Pt(style.font_size),
            font: font_id.clone(),
        });

        let y = margins.bottom - style.font_size;
        let line_width = self.layout_engine.measure_text_width(&final_text, &style);
        let content_width = page_width_pt - margins.left - margins.right;

        let mut x = margins.left;
        match style.text_align {
            TextAlign::Right => x = page_width_pt - margins.right - line_width,
            TextAlign::Center => {
                x = margins.left + (content_width - line_width) / 2.0
            }
            _ => {}
        }

        let matrix = TextMatrix::Translate(Pt(x), Pt(y));
        footer_ops.push(Op::SetTextMatrix { matrix });
        footer_ops.push(Op::WriteText {
            items: vec![TextItem::Text(final_text)],
            font: font_id.clone(),
        });
        footer_ops.push(Op::EndTextSection);

        Ok(Some(footer_ops))
    }
}

/// Manages the state and generation of PDF operations for a single page's content.
pub(super) struct PageRenderer<'a, W: io::Write + Send> {
    pub(super) doc_renderer: &'a mut PdfDocumentRenderer<W>,
    pub(super) page_height_pt: f32,
    pub(super) ops: Vec<Op>,
    pub(super) state: PageRenderState,
}

/// Tracks the current graphics state to avoid redundant PDF operations.
#[derive(Default)]
pub(crate) struct PageRenderState {
    pub(super) is_text_section_open: bool,
    // Store the owned, cloneable FontId instead of a reference.
    pub(super) current_font_id: Option<FontId>,
    pub(super) current_font_size: Option<f32>,
    pub(super) current_fill_color: Option<printpdf::color::Color>,
}

impl<'a, W: io::Write + Send> PageRenderer<'a, W> {
    fn new(doc_renderer: &'a mut PdfDocumentRenderer<W>, page_height_pt: f32) -> Self {
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

/// A bundle of read-only context needed to render a page's elements into `Op`s.
/// This is designed to be passed to parallel rendering tasks.
pub(crate) struct RenderContext<'a> {
    pub(crate) image_xobjects: &'a HashMap<String, (XObjectId, (u32, u32))>,
    pub(crate) fonts: &'a HashMap<String, FontId>,
    pub(crate) default_font: &'a FontId,
    pub(crate) page_height_pt: f32,
}

pub(crate) fn get_styled_font_name(style: &Arc<ComputedStyle>) -> String {
    let family = &style.font_family;
    match style.font_weight {
        FontWeight::Bold | FontWeight::Black => format!("{}-Bold", family),
        // This can be expanded to handle other styles like Italic if you add the fonts
        _ => family.to_string(),
    }
}

/// Renders a vector of `PositionedElement`s into a vector of PDF `Op`s.
/// This is a pure, CPU-bound function that can be run in parallel.
pub(crate) fn render_page_to_ops(
    ctx: RenderContext,
    elements: Vec<PositionedElement>,
) -> Result<Vec<Op>, RenderError> {
    let mut ops = Vec::new();
    let mut state = PageRenderState::default();

    // The drawing functions need access to the image cache, which is part of the main
    // renderer. We pass it through the context.
    for element in elements {
        drawing::draw_element_stateless(&mut ops, &mut state, &ctx, &element)?;
    }

    // Finalize the page by closing any open text sections.
    if state.is_text_section_open {
        ops.push(Op::EndTextSection);
    }
    Ok(ops)
}

/// Renders the footer for a given page and returns the PDF operations.
pub(crate) fn render_footer_to_ops<W: io::Write + Send>(
    layout_engine: &LayoutEngine,
    fonts: &HashMap<String, FontId>,
    default_font: &FontId,
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

    let style_sets = if let Some(style_name) = page_layout.footer_style.as_deref() {
        layout_engine
            .stylesheet
            .styles
            .get(style_name)
            .map(|style_arc| vec![Arc::clone(style_arc)])
            .unwrap_or_default()
    } else {
        vec![]
    };
    let default_margins = Margins::default();
    let margins = page_layout.margins.as_ref().unwrap_or(&default_margins);

    let style =
        layout_engine.compute_style(&style_sets, None, &layout_engine.get_default_style());

    let rendered_text = template_engine
        .render_template(footer_template, &footer_context)
        .map_err(|e| RenderError::TemplateError(e.to_string()))?;

    // Manual replacement for legacy placeholders for now.
    let final_text = rendered_text;

    let (page_width_pt, _) = PdfDocumentRenderer::<W>::get_page_dimensions_pt(page_layout);
    let font_id = fonts.get(style.font_family.as_str()).unwrap_or(default_font);
    let color = Rgb::new(
        style.color.r as f32 / 255.0,
        style.color.g as f32 / 255.0,
        style.color.b as f32 / 255.0,
        None,
    );

    let mut footer_ops = Vec::new();
    footer_ops.push(Op::StartTextSection);
    footer_ops.push(Op::SetFillColor {
        col: printpdf::color::Color::Rgb(color),
    });
    footer_ops.push(Op::SetFontSize {
        size: Pt(style.font_size),
        font: font_id.clone(),
    });

    let y = margins.bottom - style.font_size;
    let line_width = layout_engine.measure_text_width(&final_text, &style);
    let content_width = page_width_pt - margins.left - margins.right;

    let mut x = margins.left;
    match style.text_align {
        TextAlign::Right => x = page_width_pt - margins.right - line_width,
        TextAlign::Center => x = margins.left + (content_width - line_width) / 2.0,
        _ => {}
    }

    let matrix = TextMatrix::Translate(Pt(x), Pt(y));
    footer_ops.push(Op::SetTextMatrix { matrix });
    footer_ops.push(Op::WriteText {
        items: vec![TextItem::Text(final_text)],
        font: font_id.clone(),
    });
    footer_ops.push(Op::EndTextSection);

    Ok(Some(footer_ops))
}