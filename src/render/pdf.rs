// src/render/pdf.rs
use crate::error::RenderError;
use crate::layout::{
    ImageElement, LayoutElement, LayoutEngine, PositionedElement, RectElement, TextElement,
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

pub struct PdfDocumentRenderer<'a> {
    document: PdfDocument,
    fonts: HashMap<String, FontId>,
    default_font: FontId,
    page_contexts: Vec<&'a Value>,
    current_page_ops: Vec<Op>,
    current_page_layout: Option<PageLayout>,
    has_errored: bool,
    stylesheet: Stylesheet,
    is_text_section_open: bool,
    current_font_id: Option<FontId>,
    current_font_size: Option<f32>,
    current_fill_color: Option<printpdf::color::Color>,
    image_xobjects: HashMap<String, (XObjectId, (u32, u32))>,
    // --- NEW: Hyperlink State ---
    active_link_href: Option<String>,
}

impl<'a> PdfDocumentRenderer<'a> {
    pub fn new(stylesheet: &Stylesheet) -> Result<Self, RenderError> {
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
            current_page_ops: Vec::new(),
            current_page_layout: None,
            has_errored: false,
            stylesheet: stylesheet.clone(),
            is_text_section_open: false,
            current_font_id: None,
            current_font_size: None,
            current_fill_color: None,
            image_xobjects: HashMap::new(),
            active_link_href: None,
        })
    }

    fn close_text_section_if_open(&mut self) {
        if self.is_text_section_open {
            self.current_page_ops.push(Op::EndTextSection);
            self.is_text_section_open = false;
            self.current_font_id = None;
            self.current_font_size = None;
            self.current_fill_color = None;
        }
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

impl<'a> DocumentRenderer<'a> for PdfDocumentRenderer<'a> {
    fn begin_document(&mut self) -> Result<(), RenderError> {
        Ok(())
    }
    fn begin_page(&mut self, page_layout: &PageLayout) -> Result<(), RenderError> {
        self.end_page();
        self.current_page_layout = Some(page_layout.clone());
        Ok(())
    }

    fn end_page(&mut self) {
        self.close_text_section_if_open();
        if let Some(layout) = self.current_page_layout.take() {
            if !self.current_page_ops.is_empty() {
                let (width_mm, height_mm) = Self::get_page_dimensions_mm(&layout);
                let mut ops = Vec::new();
                let layer_name = format!("Page {} Layer 1", self.document.pages.len() + 1);
                let layer = Layer::new(&*layer_name);
                let layer_id = self.document.add_layer(&layer);
                ops.push(Op::BeginLayer { layer_id });
                ops.append(&mut self.current_page_ops);
                let pdf_page = PdfPage::new(width_mm, height_mm, ops);
                self.document.pages.push(pdf_page);
            }
        }
    }

    fn start_new_logical_page(&mut self, context: &'a Value) {
        self.page_contexts.push(context);
    }

    fn render_element(
        &mut self,
        element: &PositionedElement,
        layout_engine: &LayoutEngine,
    ) -> Result<(), RenderError> {
        if self.has_errored {
            return Ok(());
        }

        if self.active_link_href.is_some() {
            // TODO: Hyperlink Annotation Rendering is currently disabled.
            // The version of the `printpdf` library that the build environment
            // appears to be using does not have a public API for adding annotations
            // to a page (e.g., the `PdfPage::annotations` field is missing).
            // This feature cannot be fully implemented until the project's PDF
            // library dependency is updated or corrected to a version that supports this.
        }

        match &element.element {
            LayoutElement::Text(text) => {
                self.render_text(text, element, layout_engine)?;
            }
            LayoutElement::Rectangle(rect) => {
                self.render_rectangle(rect, element)?;
            }
            LayoutElement::Image(image) => {
                self.render_image(image, element)?;
            }
        }
        Ok(())
    }

    fn start_hyperlink(&mut self, href: &str) {
        self.active_link_href = Some(href.to_string());
    }

    fn end_hyperlink(&mut self) {
        self.active_link_href = None;
    }

    fn finalize<W: io::Write>(
        mut self,
        mut writer: W,
        template_engine: &Handlebars,
    ) -> Result<(), RenderError> {
        if self.has_errored {
            return Err(RenderError::Aborted);
        }
        self.end_page();

        let layout_engine = LayoutEngine::new(&self.stylesheet);
        let mut pages = std::mem::take(&mut self.document.pages);
        let total_pages = pages.len();

        for (i, page) in pages.iter_mut().enumerate() {
            let context_data = self.page_contexts.get(i).unwrap_or(&&Value::Null);

            // Create a mutable copy of the context to add page numbers
            let mut context_with_pagination = (*context_data).clone();
            if let Some(obj) = context_with_pagination.as_object_mut() {
                obj.insert("page_num".to_string(), (i + 1).into());
                obj.insert("total_pages".to_string(), total_pages.into());
            }

            let page_layout = &self.stylesheet.page;

            if let Some(footer_template) = &page_layout.footer_text {
                let mut footer_ops = Vec::new();
                let style = layout_engine
                    .compute_style_from_default(page_layout.footer_style.as_deref());

                let rendered_text = template_engine
                    .render_template(footer_template, &context_with_pagination)
                    .map_err(|e| RenderError::TemplateError(e.to_string()))?;

                // Legacy %p and %t replacement for backward compatibility, though {{page_num}} is preferred
                let final_text = rendered_text
                    .replace("%p", &(i + 1).to_string())
                    .replace("%t", &total_pages.to_string());

                let (page_width_pt, _) = Self::get_page_dimensions_pt(page_layout);
                let font_id = self.get_font(&style.font_family);
                let color = Rgb::new(
                    style.color.r as f32 / 255.0,
                    style.color.g as f32 / 255.0,
                    style.color.b as f32 / 255.0,
                    None,
                );

                footer_ops.push(Op::StartTextSection);
                footer_ops.push(Op::SetFillColor {
                    col: printpdf::color::Color::Rgb(color),
                });
                footer_ops.push(Op::SetFontSize {
                    size: Pt(style.font_size),
                    font: font_id.clone(),
                });

                let y = page_layout.margins.bottom - style.font_size;
                let mut x = page_layout.margins.left;
                if style.text_align != TextAlign::Left {
                    let line_width = layout_engine.measure_text_width(&final_text, &style);
                    let content_width =
                        page_width_pt - page_layout.margins.left - page_layout.margins.right;
                    match style.text_align {
                        TextAlign::Right => {
                            x = page_width_pt - page_layout.margins.right - line_width
                        }
                        TextAlign::Center => {
                            x = page_layout.margins.left + (content_width - line_width) / 2.0
                        }
                        _ => {}
                    }
                }
                let matrix = TextMatrix::Translate(Pt(x), Pt(y));
                footer_ops.push(Op::SetTextMatrix { matrix });
                footer_ops.push(Op::WriteText {
                    items: vec![TextItem::Text(final_text)],
                    font: font_id,
                });
                footer_ops.push(Op::EndTextSection);

                page.ops.extend(footer_ops);
            }
        }

        self.document.pages = pages;
        let mut warnings = Vec::new();
        self.document.save_writer(
            &mut writer,
            &PdfSaveOptions::default(),
            &mut warnings,
        );
        Ok(())
    }
}

impl<'a> PdfDocumentRenderer<'a> {
    fn to_pdf_color(c: &Color) -> printpdf::color::Color {
        printpdf::color::Color::Rgb(Rgb::new(
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
            None,
        ))
    }

    fn render_image(
        &mut self,
        image_el: &ImageElement,
        positioned: &PositionedElement,
    ) -> Result<(), RenderError> {
        self.close_text_section_if_open();

        let (xobj_id, (img_w, img_h)) =
            if let Some((id, dims)) = self.image_xobjects.get(&image_el.src) {
                (id.clone(), *dims)
            } else {
                let mut warnings = Vec::new();
                let raw_image = printpdf::image::RawImage::decode_from_bytes(
                    &image_el.image_data,
                    &mut warnings,
                )
                    .map_err(|e| {
                        RenderError::InternalPdfError(format!(
                            "Failed to decode image data for {}: {}",
                            image_el.src, e
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
                    .insert(image_el.src.clone(), (xobj_id.clone(), dims));
                (xobj_id, dims)
            };

        let page_layout = self.current_page_layout.as_ref().unwrap();
        let (_width_pt, page_height_pt) = Self::get_page_dimensions_pt(page_layout);

        // The position needs to be the bottom-left corner of the image
        let x = positioned.x;
        let y = page_height_pt - (positioned.y + positioned.height);

        // The `UseXobject` operation ignores the CTM and uses its own transform.
        // We set a DPI of 72, which makes one pixel equal to one point. Then,
        // we can use the scale_x/scale_y factors to scale the image from its
        // intrinsic pixel dimensions to the target point dimensions on the page.
        let transform = XObjectTransform {
            translate_x: Some(Pt(x)),
            translate_y: Some(Pt(y)),
            scale_x: Some(positioned.width / (img_w as f32)),
            scale_y: Some(positioned.height / (img_h as f32)),
            rotate: None,
            dpi: Some(72.0),
        };

        self.current_page_ops.push(Op::UseXobject {
            id: xobj_id,
            transform,
        });
        Ok(())
    }

    fn render_rectangle(
        &mut self,
        _rect: &RectElement,
        positioned: &PositionedElement,
    ) -> Result<(), RenderError> {
        self.close_text_section_if_open();

        let style = &positioned.style;
        if let Some(bg_color) = &style.background_color {
            let page_layout = self.current_page_layout.as_ref().unwrap();
            let (_width_pt, page_height_pt) = Self::get_page_dimensions_pt(page_layout);
            let x = positioned.x;
            let y = page_height_pt - (positioned.y + positioned.height);
            let width = positioned.width;
            let height = positioned.height;

            let polygon = Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        LinePoint {
                            p: Point { x: Pt(x), y: Pt(y) },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(x + width),
                                y: Pt(y),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(x + width),
                                y: Pt(y + height),
                            },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point {
                                x: Pt(x),
                                y: Pt(y + height),
                            },
                            bezier: false,
                        },
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::EvenOdd,
            };

            self.current_page_ops
                .push(Op::SetFillColor {
                    col: Self::to_pdf_color(bg_color),
                });
            self.current_page_ops.push(Op::DrawPolygon { polygon });
        }
        Ok(())
    }

    fn render_text(
        &mut self,
        text: &TextElement,
        positioned: &PositionedElement,
        layout_engine: &LayoutEngine,
    ) -> Result<(), RenderError> {
        let style = &positioned.style;
        let font_id = self.get_font(&style.font_family);
        let fill_color = Self::to_pdf_color(&style.color);

        if let Some(bg_color) = &style.background_color {
            self.close_text_section_if_open();
            // This is a simplified version of rectangle drawing.
            // A full implementation would be in `render_rectangle`.
            // Here, we just draw the background for this specific text element.
            let page_layout = self.current_page_layout.as_ref().unwrap();
            let (_width_pt, page_height_pt) = Self::get_page_dimensions_pt(page_layout);
            let x = positioned.x;
            let y = page_height_pt - (positioned.y + positioned.height);
            let width = positioned.width;
            let height = positioned.height;
            let polygon = Polygon { rings: vec![PolygonRing { points: vec![ LinePoint { p: Point { x: Pt(x), y: Pt(y) }, bezier: false }, LinePoint { p: Point { x: Pt(x + width), y: Pt(y) }, bezier: false }, LinePoint { p: Point { x: Pt(x + width), y: Pt(y + height) }, bezier: false }, LinePoint { p: Point { x: Pt(x), y: Pt(y + height) }, bezier: false }, ], }], mode: PaintMode::Fill, winding_order: WindingOrder::EvenOdd, };
            self.current_page_ops
                .push(Op::SetFillColor {
                    col: Self::to_pdf_color(bg_color),
                });
            self.current_page_ops.push(Op::DrawPolygon { polygon });
        }

        if !self.is_text_section_open {
            self.current_page_ops.push(Op::StartTextSection);
            self.is_text_section_open = true;
            self.current_fill_color = None;
            self.current_font_id = None;
            self.current_font_size = None;
        }

        if self.current_fill_color.as_ref() != Some(&fill_color) {
            self.current_page_ops
                .push(Op::SetFillColor {
                    col: fill_color.clone(),
                });
            self.current_fill_color = Some(fill_color);
        }

        if self.current_font_id.as_ref() != Some(&font_id)
            || self.current_font_size != Some(style.font_size)
        {
            self.current_page_ops.push(Op::SetFontSize {
                size: Pt(style.font_size),
                font: font_id.clone(),
            });
            self.current_font_id = Some(font_id.clone());
            self.current_font_size = Some(style.font_size);
        }

        let page_layout = self.current_page_layout.as_ref().unwrap();
        let (_width_pt, page_height_pt) = Self::get_page_dimensions_pt(page_layout);
        let content_width = positioned.width - style.padding.left - style.padding.right;
        let lines = layout_engine.wrap_text(&text.content, style, content_width);

        for (i, line) in lines.iter().enumerate() {
            let mut x = positioned.x + style.padding.left;
            let line_top_y = positioned.y + style.padding.top + (i as f32 * style.line_height);

            if style.text_align != TextAlign::Left {
                let line_width = layout_engine.measure_text_width(line, style);

                match style.text_align {
                    TextAlign::Right => {
                        x = (positioned.x + positioned.width - style.padding.right) - line_width
                    }
                    TextAlign::Center => {
                        x = positioned.x + style.padding.left + (content_width - line_width) / 2.0
                    }
                    _ => {}
                }
            }

            let baseline_y = line_top_y + style.font_size * 0.8;
            let pdf_y = page_height_pt - baseline_y;
            let matrix = TextMatrix::Translate(Pt(x), Pt(pdf_y));
            self.current_page_ops.push(Op::SetTextMatrix { matrix });
            self.current_page_ops.push(Op::WriteText {
                items: vec![TextItem::Text(line.clone())],
                font: font_id.clone(),
            });
        }

        Ok(())
    }
}