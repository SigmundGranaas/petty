// src/render/drawing/rect.rs
// src/render/drawing/rect.rs

use super::super::pdf::PageRenderer;
use crate::error::RenderError;
use crate::layout::PositionedElement;
use crate::stylesheet::Color;
use printpdf::graphics::{LinePoint, PaintMode, Point, Polygon, PolygonRing, WindingOrder};
use printpdf::ops::Op;
use printpdf::{Pt, Rgb};

/// A helper to convert our internal `Color` to the `printpdf` library's `Color`.
fn to_pdf_color(c: &Color) -> printpdf::color::Color {
    printpdf::color::Color::Rgb(Rgb::new(
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        None,
    ))
}

/// Renders the background color and borders for any `PositionedElement`.
pub(super) fn draw_background_and_borders(
    page: &mut PageRenderer,
    positioned: &PositionedElement,
) -> Result<(), RenderError> {
    // Rectangles cannot be drawn within a text section.
    if page.state.is_text_section_open {
        page.ops.push(Op::EndTextSection);
        page.state.is_text_section_open = false;
    }

    let style = &positioned.style;
    let x = positioned.x;
    let y = page.page_height_pt - (positioned.y + positioned.height);
    let width = positioned.width;
    let height = positioned.height;

    // Render background color if present.
    if let Some(bg_color) = &style.background_color {
        let polygon = Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint { p: Point { x: Pt(x), y: Pt(y) }, bezier: false },
                    LinePoint { p: Point { x: Pt(x + width), y: Pt(y) }, bezier: false },
                    LinePoint { p: Point { x: Pt(x + width), y: Pt(y + height) }, bezier: false },
                    LinePoint { p: Point { x: Pt(x), y: Pt(y + height) }, bezier: false },
                ],
            }],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::EvenOdd,
        };
        page.ops.push(Op::SetFillColor { col: to_pdf_color(bg_color) });
        page.ops.push(Op::DrawPolygon { polygon });
    }

    // Render border-bottom if present.
    if let Some(border) = &style.border_bottom {
        page.ops.push(Op::SetOutlineThickness { pt: Pt(border.width) });
        page.ops.push(Op::SetOutlineColor { col: to_pdf_color(&border.color) });
        let line_y = page.page_height_pt - (positioned.y + positioned.height);
        let line = Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    LinePoint { p: Point { x: Pt(positioned.x), y: Pt(line_y) }, bezier: false },
                    LinePoint { p: Point { x: Pt(positioned.x + positioned.width), y: Pt(line_y) }, bezier: false },
                ],
            }],
            mode: PaintMode::Stroke,
            winding_order: WindingOrder::EvenOdd,
        };
        page.ops.push(Op::DrawPolygon { polygon: line });
    }

    Ok(())
}