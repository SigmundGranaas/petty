use super::super::pdf::{PageRenderState, PageRenderer, RenderContext};
// Add this use statement to bring the helper into scope
use crate::render::pdf::get_styled_font_name;
use printpdf::ops::Op;
use printpdf::{Pt, Rgb, TextItem, TextMatrix};
use std::io;
use crate::core::layout::{PositionedElement, TextElement};
use crate::render::RenderError;

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

    let styled_font_name = get_styled_font_name(style);
    let font_id = match page.doc_renderer.fonts.get(&styled_font_name) {
        Some(font) => font,
        None => {
            if styled_font_name != style.font_family.as_str() {
                log::warn!(
                    "Font style '{}' not found for rendering, falling back to base font '{}'.",
                    styled_font_name, style.font_family
                );
            }
            page.doc_renderer
                .fonts
                .get(style.font_family.as_str())
                .unwrap_or(&page.doc_renderer.default_font)
        }
    };

    let fill_color = printpdf::color::Color::Rgb(Rgb::new(
        style.color.r as f32 / 255.0,
        style.color.g as f32 / 255.0,
        style.color.b as f32 / 255.0,
        None,
    ));

    if !page.state.is_text_section_open {
        page.ops.push(Op::StartTextSection);
        page.state.is_text_section_open = true;
    }
    if page.state.current_fill_color.as_ref() != Some(&fill_color) {
        page.ops.push(Op::SetFillColor { col: fill_color.clone() });
        page.state.current_fill_color = Some(fill_color);
    }
    if page.state.current_font_id.as_ref() != Some(font_id)
        || page.state.current_font_size != Some(style.font_size)
    {
        page.ops.push(Op::SetFontSize {
            size: Pt(style.font_size),
            font: font_id.clone(),
        });
        page.state.current_font_id = Some(font_id.clone());
        page.state.current_font_size = Some(style.font_size);
    }

    let line_height = positioned.height;
    let font_size = style.font_size;
    // Heuristic to vertically center the font's em-box within the line-box and find the baseline.
    // This better respects the `line-height` property from the stylesheet.
    let leading = line_height - font_size;
    let ascent_approx = font_size * 0.8; // A common heuristic for font ascent.
    let baseline_y = positioned.y + (leading / 2.0) + ascent_approx;

    let pdf_y = page.page_height_pt - baseline_y;
    let matrix = TextMatrix::Translate(Pt(positioned.x), Pt(pdf_y));
    page.ops.push(Op::SetTextMatrix { matrix });
    page.ops.push(Op::WriteText {
        items: vec![TextItem::Text(text.content.clone())],
        font: font_id.clone(),
    });

    Ok(())
}

/// Renders a `TextElement` statelessly using font info from the context.
pub(super) fn draw_text_stateless(
    ops: &mut Vec<Op>,
    state: &mut PageRenderState,
    ctx: &RenderContext,
    text: &TextElement,
    positioned: &PositionedElement,
) -> Result<(), RenderError> {
    if text.content.is_empty() {
        return Ok(());
    }

    let style = &positioned.style;

    let styled_font_name = get_styled_font_name(style);
    let font_id = match ctx.fonts.get(&styled_font_name) {
        Some(font) => font,
        None => {
            if styled_font_name != style.font_family.as_str() {
                log::warn!(
                    "Font style '{}' not found for rendering, falling back to base font '{}'.",
                    styled_font_name, style.font_family
                );
            }
            ctx.fonts.get(style.font_family.as_str()).unwrap_or(ctx.default_font)
        }
    };

    let fill_color = printpdf::color::Color::Rgb(Rgb::new(
        style.color.r as f32 / 255.0,
        style.color.g as f32 / 255.0,
        style.color.b as f32 / 255.0,
        None,
    ));

    if !state.is_text_section_open {
        ops.push(Op::StartTextSection);
        state.is_text_section_open = true;
    }
    if state.current_fill_color.as_ref() != Some(&fill_color) {
        ops.push(Op::SetFillColor { col: fill_color.clone() });
        state.current_fill_color = Some(fill_color);
    }
    if state.current_font_id.as_ref() != Some(font_id)
        || state.current_font_size != Some(style.font_size)
    {
        ops.push(Op::SetFontSize {
            size: Pt(style.font_size),
            font: font_id.clone(),
        });
        state.current_font_id = Some(font_id.clone());
        state.current_font_size = Some(style.font_size);
    }

    let line_height = positioned.height;
    let font_size = style.font_size;
    let leading = line_height - font_size;
    let ascent_approx = font_size * 0.8;
    let baseline_y = positioned.y + (leading / 2.0) + ascent_approx;

    let pdf_y = ctx.page_height_pt - baseline_y;
    let matrix = TextMatrix::Translate(Pt(positioned.x), Pt(pdf_y));
    ops.push(Op::SetTextMatrix { matrix });
    ops.push(Op::WriteText {
        items: vec![TextItem::Text(text.content.clone())],
        font: font_id.clone(),
    });

    Ok(())
}