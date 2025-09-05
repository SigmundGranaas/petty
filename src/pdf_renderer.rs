use crate::layout_engine::{LayoutElement, LayoutEngine, Page, PositionedElement, TableElement, TextElement};
use crate::stylesheet::{Border, BorderStyle, Color, PageLayout, TextAlign};
use printpdf::{FontId, Layer, LineDashPattern, Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, Point, Pt, Rgb, PdfConformance, TextMatrix, PaintMode};
use printpdf::font::ParsedFont;
use printpdf::graphics::{LinePoint, Polygon, PolygonRing, WindingOrder};
use printpdf::text::{TextItem};

pub struct PdfRenderer {
    document: PdfDocument,
    fonts: FontRegistry,
}

pub struct FontRegistry {
    default_font: FontId,
    bold_font: Option<FontId>,
    italic_font: Option<FontId>,
    fonts: std::collections::HashMap<String, FontId>,
}

impl PdfRenderer {
    pub fn new(title: &str) -> Self {
        let mut doc = PdfDocument::new(title);
        doc.metadata.info.conformance = PdfConformance::X3_2002_PDF_1_3;

        let font_data = include_bytes!("../assets/fonts/Helvetica.ttf");
        let mut warnings = Vec::new();
        let font = ParsedFont::from_bytes(font_data, 0, &mut warnings).unwrap();
        let default_font = doc.add_font(&font);

        let fonts = FontRegistry {
            default_font,
            bold_font: None,
            italic_font: None,
            fonts: std::collections::HashMap::new(),
        };

        PdfRenderer {
            document: doc,
            fonts,
        }
    }

    pub fn render(&mut self, pages: &[Page], page_layout: &PageLayout, layout_engine: &LayoutEngine) -> Result<Vec<u8>, PdfError> {
        self.document.pages.clear();
        for (idx, page) in pages.iter().enumerate() {
            self.render_page(page, page_layout, idx, layout_engine)?;
        }
        let mut warnings = Vec::new();
        Ok(self.document.save(&PdfSaveOptions::default(), &mut warnings))
    }

    fn render_page(&mut self, page: &Page, page_layout: &PageLayout, page_idx: usize, layout_engine: &LayoutEngine) -> Result<(), PdfError> {
        let (width_mm, height_mm) = self.get_page_dimensions_mm(page_layout);

        let mut ops: Vec<Op> = Vec::new();
        let layer_name = format!("Page {} Layer 1", page_idx + 1);
        let layer = Layer::new(&*layer_name);
        let layer_id = self.document.add_layer(&layer);

        ops.push(Op::BeginLayer { layer_id });

        for element in &page.elements {
            let element_ops = self.render_element(element, page_layout, layout_engine)?;
            ops.extend(element_ops);
        }

        let pdf_page = PdfPage::new(width_mm, height_mm, ops);
        self.document.pages.push(pdf_page);
        Ok(())
    }

    fn render_element(&mut self, element: &PositionedElement, page_layout: &PageLayout, layout_engine: &LayoutEngine) -> Result<Vec<Op>, PdfError> {
        let mut ops = Vec::new();
        if let Some(bg_color) = &element.style.background_color {
            ops.extend(self.draw_rectangle(element.x, element.y, element.width, element.height, Some(bg_color), None, page_layout)?);
        }

        match &element.element {
            LayoutElement::Text(text) => {
                ops.extend(self.render_text(text, element, page_layout)?);
            }
            LayoutElement::Image(_image) => { /* Image rendering not yet implemented */ }
            LayoutElement::Table(table) => {
                ops.extend(self.render_table(table, element, page_layout, layout_engine)?);
            }
            LayoutElement::Container(_container) => {
                // The container itself is just a box. Its children are separate PositionedElements
                // that will be rendered by the main loop. We just need to draw its border here.
            }
            LayoutElement::Rectangle(_rect) => {
                // The background was already drawn if specified in the style.
                // A rectangle element is primarily for its styled box model.
            }
        }

        // Draw border on top of content
        if let Some(border) = &element.style.border {
            ops.extend(self.draw_rectangle(element.x, element.y, element.width, element.height, None, Some(border), page_layout)?);
        }
        Ok(ops)
    }

    fn render_text(&mut self, text: &TextElement, positioned: &PositionedElement, page_layout: &PageLayout) -> Result<Vec<Op>, PdfError> {
        let mut ops = Vec::new();
        let style = &positioned.style;
        let font_id = self.get_font(&style.font_family);
        let color = Rgb::new(style.color.r as f32 / 255.0, style.color.g as f32 / 255.0, style.color.b as f32 / 255.0, None);
        ops.push(Op::SetFillColor { col: printpdf::color::Color::Rgb(color) });

        let (_width_mm, height_mm) = self.get_page_dimensions_mm(page_layout);
        let page_height_pt = height_mm.into_pt().0;

        ops.push(Op::StartTextSection);
        ops.push(Op::SetFontSize { size: Pt(style.font_size), font: font_id.clone() });

        for line in &text.lines {
            let mut x = line.x;
            if style.text_align != TextAlign::Left {
                let char_width_approx = style.font_size * 0.6;
                let line_width = line.text.len() as f32 * char_width_approx;

                match style.text_align {
                    TextAlign::Right => x = (line.x + line.width) - line_width,
                    TextAlign::Center => x = line.x + (line.width - line_width) / 2.0,
                    _ => {}
                }
            }

            let pdf_y = page_height_pt - line.y - style.font_size;
            let matrix = TextMatrix::Translate(Pt(x), Pt(pdf_y));
            ops.push(Op::SetTextMatrix { matrix });
            ops.push(Op::WriteText { items: vec![TextItem::Text(line.text.clone())], font: font_id.clone() });
        }
        ops.push(Op::EndTextSection);
        Ok(ops)
    }

    fn render_table(&mut self, table: &TableElement, element: &PositionedElement, page_layout: &PageLayout, layout_engine: &LayoutEngine) -> Result<Vec<Op>, PdfError> {
        let mut ops = Vec::new();
        let mut y_offset = 0.0;

        for row in table.rows.iter() {
            let mut x_offset = 0.0;
            for (col_idx, cell) in row.cells.iter().enumerate() {
                let cell_width = *table.column_widths.get(col_idx).unwrap_or(&100.0);
                let cell_x = element.x + x_offset;
                let cell_y = element.y + y_offset;

                if let LayoutElement::Text(text_elem) = &*cell.content {
                    let style_name = text_elem.style_name.as_ref().map(|s| s.as_str());
                    let cell_style = layout_engine.compute_style(style_name);

                    // Draw cell background and border based on the cell's own style
                    ops.extend(self.draw_rectangle(cell_x, cell_y, cell_width, row.height, cell_style.background_color.as_ref(), cell_style.border.as_ref(), page_layout)?);

                    // Render text content within the cell
                    let content_width = cell_width - cell_style.padding.left - cell_style.padding.right;
                    let lines = layout_engine.wrap_text(&text_elem.content, &cell_style, content_width);

                    // Vertically center the text block within the cell's content area
                    let total_text_height = lines.len() as f32 * cell_style.line_height;
                    let available_content_height = row.height - cell_style.padding.top - cell_style.padding.bottom;
                    let text_y_start_offset = ((available_content_height - total_text_height) / 2.0).max(0.0);

                    let mut line_y_offset = 0.0;
                    for line_text in lines {
                        let text_line = crate::layout_engine::TextLine {
                            text: line_text.clone(),
                            x: cell_x + cell_style.padding.left,
                            y: cell_y + cell_style.padding.top + text_y_start_offset + line_y_offset,
                            width: content_width,
                            height: cell_style.line_height
                        };

                        let positioned_cell_text = PositionedElement {
                            x: text_line.x,
                            y: text_line.y,
                            width: content_width,
                            height: cell_style.line_height,
                            element: LayoutElement::Text(TextElement {
                                style_name: text_elem.style_name.clone(),
                                content: line_text,
                                lines: vec![text_line],
                            }),
                            style: cell_style.clone(),
                        };

                        if let LayoutElement::Text(text_content) = &positioned_cell_text.element {
                            ops.extend(self.render_text(text_content, &positioned_cell_text, page_layout)?);
                        }
                        line_y_offset += cell_style.line_height;
                    }
                }
                x_offset += cell_width;
            }
            y_offset += row.height;
        }
        Ok(ops)
    }

    fn draw_rectangle(&self, x: f32, y: f32, width: f32, height: f32, fill: Option<&Color>, border: Option<&Border>, page_layout: &PageLayout) -> Result<Vec<Op>, PdfError> {
        let has_fill = fill.is_some();
        let has_stroke = border.is_some();
        if !has_fill && !has_stroke { return Ok(Vec::new()); }

        let mut ops = Vec::new();
        let (_width_mm, height_mm) = self.get_page_dimensions_mm(page_layout);
        let page_height_pt = height_mm.into_pt().0;
        let pdf_y = page_height_pt - y - height;

        let p1 = Point { x: Pt(x), y: Pt(pdf_y) };
        let p2 = Point { x: Pt(x + width), y: Pt(pdf_y) };
        let p3 = Point { x: Pt(x + width), y: Pt(pdf_y + height) };
        let p4 = Point { x: Pt(x), y: Pt(pdf_y + height) };
        let points = vec![LinePoint{p:p1, bezier:false}, LinePoint{p:p2, bezier:false}, LinePoint{p:p3, bezier:false}, LinePoint{p:p4, bezier:false}];

        if let Some(fill_color) = fill {
            let color = Rgb::new(fill_color.r as f32 / 255.0, fill_color.g as f32 / 255.0, fill_color.b as f32 / 255.0, None);
            ops.push(Op::SetFillColor { col: printpdf::color::Color::Rgb(color) });
        }

        if let Some(border) = border {
            let color = Rgb::new(border.color.r as f32 / 255.0, border.color.g as f32 / 255.0, border.color.b as f32 / 255.0, None);
            ops.push(Op::SetOutlineColor { col: printpdf::color::Color::Rgb(color) });
            ops.push(Op::SetOutlineThickness { pt: Pt(border.width) });
            if let BorderStyle::Dashed = border.style {
                ops.push(Op::SetLineDashPattern { dash: LineDashPattern { dash_1: Some(3), gap_1: Some(3), dash_2: None, gap_2: None, dash_3: None, gap_3: None, offset: 0 } });
            }
        }

        let mut paint_mode = PaintMode::Fill;
        if has_fill && has_stroke {
            paint_mode = PaintMode::FillStroke;
        } else if has_stroke {
            paint_mode = PaintMode::Stroke;
        }

        ops.push(Op::DrawPolygon { polygon: Polygon { rings: vec![PolygonRing { points: points.clone() }], mode: paint_mode, winding_order: WindingOrder::NonZero } });

        if has_stroke {
            if let Some(b) = border {
                if matches!(b.style, BorderStyle::Dashed) {
                    ops.push(Op::SetLineDashPattern { dash: LineDashPattern::default() });
                }
            }
        }
        Ok(ops)
    }

    fn get_font(&self, font_family: &str) -> FontId {
        self.fonts.fonts.get(font_family).cloned().unwrap_or_else(|| self.fonts.default_font.clone())
    }

    fn get_page_dimensions_mm(&self, page_layout: &PageLayout) -> (Mm, Mm) {
        match page_layout.size {
            crate::stylesheet::PageSize::A4 => (Mm(210.0), Mm(297.0)),
            crate::stylesheet::PageSize::Letter => (Mm(215.9), Mm(279.4)),
            crate::stylesheet::PageSize::Legal => (Mm(215.9), Mm(355.6)),
            crate::stylesheet::PageSize::Custom { width, height } => (Pt(width).into(), Pt(height).into()),
        }
    }
}

#[derive(Debug)]
pub enum PdfError {
    IoError(std::io::Error),
}

impl From<std::io::Error> for PdfError {
    fn from(e: std::io::Error) -> Self {
        PdfError::IoError(e)
    }
}