// src/render/drawing/text.rs
use super::super::pdf::PageRenderer;
use crate::error::RenderError;
use crate::layout::{PositionedElement, TextElement};
use printpdf::ops::Op;
use printpdf::{Pt, Rgb, TextItem, TextMatrix};
use std::io;

/// Renders a `TextElement` to the page, managing the text section state.
pub(super) fn draw_text<W: io::Write + Send>(
    page: &mut PageRenderer<W>,
    text: &TextElement,
    positioned: &PositionedElement,
) -> Result<(), RenderError> {
    if text.content.is_empty() {
        return Ok(());
    }

    let style = &positioned.style;
    let font_id = page
        .doc_renderer
        .fonts
        .get(style.font_family.as_str())
        .unwrap_or(&page.doc_renderer.default_font);
    let fill_color = printpdf::color::Color::Rgb(Rgb::new(
        style.color.r as f32 / 255.0,
        style.color.g as f32 / 255.0,
        style.color.b as f32 / 255.0,
        None,
    ));

    // --- State Management ---
    // Ensure we are inside a text section.
    if !page.state.is_text_section_open {
        page.ops.push(Op::StartTextSection);
        page.state.is_text_section_open = true;
    }

    // Set fill color if it has changed.
    if page.state.current_fill_color.as_ref() != Some(&fill_color) {
        page.ops.push(Op::SetFillColor { col: fill_color.clone() });
        page.state.current_fill_color = Some(fill_color);
    }

    // Set font and size if they have changed.
    if page.state.current_font_id.as_ref() != Some(font_id)
        || page.state.current_font_size != Some(style.font_size)
    {
        page.ops.push(Op::SetFontSize {
            size: Pt(style.font_size),
            font: font_id.clone(),
        });
        // Store a clone of the FontId, not a reference.
        page.state.current_font_id = Some(font_id.clone());
        page.state.current_font_size = Some(style.font_size);
    }

    // --- Drawing Operations ---
    let baseline_y = positioned.y + style.font_size * 0.8;
    let pdf_y = page.page_height_pt - baseline_y;

    let matrix = TextMatrix::Translate(Pt(positioned.x), Pt(pdf_y));
    page.ops.push(Op::SetTextMatrix { matrix });
    page.ops.push(Op::WriteText {
        items: vec![TextItem::Text(text.content.clone())],
        font: font_id.clone(),
    });

    Ok(())
}