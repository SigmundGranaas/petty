// src/render/drawing/mod.rs
// src/render/drawing/mod.rs

//! Low-level functions for converting `LayoutElement`s into PDF `Op`s.

use super::pdf::PageRenderer;
use crate::error::RenderError;
use crate::layout::{LayoutElement, PositionedElement};
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
    }
    Ok(())
}