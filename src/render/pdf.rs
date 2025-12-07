use super::{drawing, renderer};
use crate::core::idf::SharedData;
use crate::core::layout::{ComputedStyle, LayoutEngine, PositionedElement};
use crate::core::style::font::FontWeight;
use crate::core::style::stylesheet::{PageLayout, Stylesheet};
use crate::render::DocumentRenderer;
use handlebars::Handlebars;
use lopdf::ObjectId;
use printpdf::font::ParsedFont;
use printpdf::image::RawImage;
use printpdf::ops::Op;
use printpdf::xobject::XObject;
use printpdf::{
    FontId, Layer, Mm, PdfConformance, PdfDocument, PdfPage, PdfSaveOptions, Pt, XObjectId,
};
use serde::Serialize;
use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::io::{self, Seek, Write};
use std::sync::Arc;

/// Manages the state of the entire PDF document, including pages, fonts, and global resources.
pub struct PdfDocumentRenderer<W: io::Write + Send> {
    pub(crate) document: PdfDocument,
    pub(crate) fonts: HashMap<String, FontId>,
    pub(crate) default_font: FontId,
    pub(crate) stylesheet: Arc<Stylesheet>,
    pub(crate) image_xobjects: HashMap<String, (XObjectId, (u32, u32))>,
    pub(crate) layout_engine: LayoutEngine,
    pub(crate) writer: Option<W>,
    buffered_page_ops: Vec<Vec<Op>>,
}

impl<W: io::Write + Send> PdfDocumentRenderer<W> {
    /// Creates a new document renderer.
    pub fn new(
        layout_engine: LayoutEngine,
        stylesheet: Arc<Stylesheet>,
    ) -> Result<Self, renderer::RenderError> {
        let title = "Document";
        let mut doc = PdfDocument::new(title);
        doc.metadata.info.conformance = PdfConformance::X3_2002_PDF_1_3;

        let mut fonts = HashMap::new();
        let mut default_font_id: Option<FontId> = None;

        // Use RefCell borrow via accessor
        let system = layout_engine.font_system();

        for face in system.db().faces() {
            let face_post_script_name = face.post_script_name.clone();
            let face_index = face.index;
            let id = face.id;

            system.db().with_face_data(id, |font_data, _| {
                let mut warnings = Vec::new();
                match ParsedFont::from_bytes(font_data, face_index as usize, &mut warnings) {
                    Some(parsed_font) => {
                        let font_id = doc.add_font(&parsed_font);
                        fonts.insert(face_post_script_name.clone(), font_id.clone());
                        if face_post_script_name.eq_ignore_ascii_case("helvetica") {
                            default_font_id = Some(font_id);
                        }
                    }
                    None => {
                        log::warn!(
                            "Failed to parse font for embedding: {}",
                            face_post_script_name
                        );
                    }
                }
            });
        }

        let default_font = default_font_id
            .or_else(|| fonts.values().next().cloned())
            .ok_or_else(|| {
                renderer::RenderError::InternalPdfError(
                    "No fonts were loaded, cannot create PDF.".to_string(),
                )
            })?;

        drop(system);

        Ok(PdfDocumentRenderer {
            document: doc,
            fonts,
            default_font,
            stylesheet,
            image_xobjects: HashMap::new(),
            layout_engine,
            writer: None,
            buffered_page_ops: Vec::new(),
        })
    }

    pub(crate) fn add_image_xobject(
        &mut self,
        src: &str,
        image_data: &SharedData,
    ) -> Result<(XObjectId, (u32, u32)), renderer::RenderError> {
        let mut warnings = Vec::new();
        let raw_image = RawImage::decode_from_bytes(image_data, &mut warnings).map_err(|e| {
            renderer::RenderError::InternalPdfError(format!(
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

    pub(crate) fn get_page_dimensions_mm(page_layout: &PageLayout) -> (Mm, Mm) {
        let (w, h) = page_layout.size.dimensions_pt();
        (Pt(w).into(), Pt(h).into())
    }
}

impl<W: Write + Seek + Send + 'static> DocumentRenderer<W> for PdfDocumentRenderer<W> {
    fn begin_document(&mut self, writer: W) -> Result<(), renderer::RenderError> {
        self.writer = Some(writer);
        Ok(())
    }

    fn add_resources(
        &mut self,
        resources: &HashMap<String, SharedData>,
    ) -> Result<(), renderer::RenderError> {
        for (src, data) in resources {
            if !self.image_xobjects.contains_key(src) {
                self.add_image_xobject(src, data)?;
            }
        }
        Ok(())
    }

    fn render_page_content(
        &mut self,
        elements: Vec<PositionedElement>,
        _font_map: &HashMap<String, String>,
        _page_width_pt: f32,
        page_height_pt: f32,
    ) -> Result<ObjectId, renderer::RenderError> {
        let ctx = RenderContext {
            image_xobjects: &self.image_xobjects,
            fonts: &self.fonts,
            default_font: &self.default_font,
            page_height_pt,
        };
        let ops = render_page_to_ops(ctx, elements)?;
        let content_id = self.buffered_page_ops.len();
        self.buffered_page_ops.push(ops);
        Ok((content_id as u32, 0))
    }

    fn write_page_object(
        &mut self,
        content_stream_ids: Vec<ObjectId>,
        _annotations: Vec<ObjectId>,
        page_width_pt: f32,
        page_height_pt: f32,
    ) -> Result<ObjectId, renderer::RenderError> {
        let mut final_ops = Vec::new();
        for (content_id, _) in content_stream_ids {
            if let Some(ops) = self.buffered_page_ops.get(content_id as usize) {
                final_ops.extend(ops.clone());
            }
        }

        let width_mm = Pt(page_width_pt).into();
        let height_mm = Pt(page_height_pt).into();

        let page_num = self.document.pages.len() + 1;
        let layer_name = format!("Page {} Layer 1", page_num);
        let layer = Layer::new(&*layer_name);
        let layer_id = self.document.add_layer(&layer);

        let mut ops_with_layer = vec![Op::BeginLayer { layer_id }];
        ops_with_layer.extend(final_ops);

        let pdf_page = PdfPage::new(width_mm, height_mm, ops_with_layer);
        self.document.pages.push(pdf_page);
        let page_id = self.document.pages.len();

        Ok((page_id as u32, 0))
    }

    fn set_outline_root(&mut self, _outline_root_id: ObjectId) {}

    fn finish(self: Box<Self>, _page_ids: Vec<ObjectId>) -> Result<W, renderer::RenderError> {
        let mut writer = self.writer.ok_or_else(|| {
            renderer::RenderError::Other(
                "Document was never started with begin_document".into(),
            )
        })?;

        let save_options = PdfSaveOptions::default();
        let mut warnings = Vec::new();

        self.document
            .save_writer(&mut writer, &save_options, &mut warnings);

        if !warnings.is_empty() {
            log::warn!(
                "printpdf generated {} warnings during save: {:?}",
                warnings.len(),
                warnings
            );
        }

        Ok(writer)
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Serialize)]
struct FooterRenderContext<'a> {
    #[serde(flatten)]
    data: &'a Value,
    page_num: usize,
    total_pages: &'static str,
}

pub(super) struct PageRenderer<'a, W: io::Write + Send> {
    pub(super) doc_renderer: &'a mut PdfDocumentRenderer<W>,
    pub(super) page_height_pt: f32,
    pub(super) ops: Vec<Op>,
    pub(super) state: PageRenderState,
}

#[derive(Default)]
pub(crate) struct PageRenderState {
    pub(super) is_text_section_open: bool,
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

    fn render_elements(
        &mut self,
        elements: Vec<PositionedElement>,
    ) -> Result<(), renderer::RenderError> {
        for element in elements {
            drawing::draw_element(self, &element)?;
        }
        Ok(())
    }

    fn into_ops(mut self) -> Vec<Op> {
        if self.state.is_text_section_open {
            self.ops.push(Op::EndTextSection);
        }
        self.ops
    }
}

pub(crate) struct RenderContext<'a> {
    pub(crate) image_xobjects: &'a HashMap<String, (XObjectId, (u32, u32))>,
    pub(crate) fonts: &'a HashMap<String, FontId>,
    pub(crate) default_font: &'a FontId,
    pub(crate) page_height_pt: f32,
}

pub(crate) fn get_styled_font_name(style: &Arc<ComputedStyle>) -> String {
    let family = &style.text.font_family;
    match style.text.font_weight {
        FontWeight::Bold | FontWeight::Black => format!("{}-Bold", family),
        _ => family.to_string(),
    }
}

pub(crate) fn render_page_to_ops(
    ctx: RenderContext,
    elements: Vec<PositionedElement>,
) -> Result<Vec<Op>, renderer::RenderError> {
    let mut ops = Vec::new();
    let mut state = PageRenderState::default();
    for element in elements {
        drawing::draw_element_stateless(&mut ops, &mut state, &ctx, &element)?;
    }
    if state.is_text_section_open {
        ops.push(Op::EndTextSection);
    }
    Ok(ops)
}

pub(crate) fn render_footer_to_ops(
    layout_engine: &LayoutEngine,
    stylesheet: &Stylesheet,
    fonts: &HashMap<String, FontId>,
    default_font: &FontId,
    _context: &Value,
    page_layout: &PageLayout,
    page_num: usize,
    template_engine: &Handlebars,
) -> Result<Option<Vec<Op>>, renderer::RenderError> {
    let _ = (
        layout_engine,
        stylesheet,
        fonts,
        default_font,
        _context,
        page_layout,
        page_num,
        template_engine,
    );
    Ok(None)
}