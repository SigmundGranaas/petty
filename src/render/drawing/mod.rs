use super::pdf::{PageRenderState, PageRenderer, RenderContext};
use crate::core::layout::{LayoutElement, PositionedElement};
use crate::render::RenderError;
use printpdf::Op;
use std::io;

pub(super) mod image;
pub(super) mod rect;
pub(super) mod text;

/// The main dispatcher for drawing any `LayoutElement`.
pub(super) fn draw_element<W: io::Write + Send>(
    page: &mut PageRenderer<W>,
    element: &PositionedElement,
) -> Result<(), RenderError> {
    // Background and borders are drawn first, underneath the content.
    rect::draw_background_and_borders(page, element)?;

    // Dispatch to the appropriate content drawing function.
    match &element.element {
        LayoutElement::Text(text_el) => text::draw_text(page, text_el, element)?,
        LayoutElement::Image(image_el) => image::draw_image(page, image_el, element)?,
        LayoutElement::Rectangle(_) => { /* Content is the background, already handled */ }
        LayoutElement::PageNumberPlaceholder { .. } => { /* Rendered in finalize step */ }
    }
    Ok(())
}

/// A stateless version of `draw_element` for use in parallel rendering.
/// It takes `ops`, `state`, and `context` directly instead of a `PageRenderer`.
pub(super) fn draw_element_stateless(
    ops: &mut Vec<Op>,
    state: &mut PageRenderState,
    ctx: &RenderContext,
    element: &PositionedElement,
) -> Result<(), RenderError> {
    // Background and borders are drawn first.
    rect::draw_background_and_borders_stateless(ops, state, ctx, element)?;

    // Dispatch to the appropriate content drawing function.
    match &element.element {
        LayoutElement::Text(text_el) => {
            text::draw_text_stateless(ops, state, ctx, text_el, element)?
        }
        LayoutElement::Image(image_el) => {
            image::draw_image_stateless(ops, state, ctx, image_el, element)?
        }
        LayoutElement::Rectangle(_) => { /* Content is the background, already handled */ }
        LayoutElement::PageNumberPlaceholder { .. } => { /* Rendered in finalize step */ }
    }
    Ok(())
}