// src/render/drawing/image.rs
// src/render/drawing/image.rs

use super::super::pdf::PageRenderer;
use crate::error::RenderError;
use crate::layout::{ImageElement, PositionedElement};
use printpdf::ops::Op;
use printpdf::xobject::XObjectTransform;
use printpdf::Pt;

/// Renders an `ImageElement` to the page.
pub(super) fn draw_image(
    page: &mut PageRenderer,
    image_el: &ImageElement,
    positioned: &PositionedElement,
) -> Result<(), RenderError> {
    // Images cannot be drawn within a text section.
    if page.state.is_text_section_open {
        page.ops.push(Op::EndTextSection);
        page.state.is_text_section_open = false;
    }

    // Check if this image has been cached as an XObject in the document.
    let (xobj_id, (img_w, img_h)) =
        if let Some(cached) = page.doc_renderer.image_xobjects.get(&image_el.src) {
            (cached.0.clone(), cached.1)
        } else {
            // If not cached, call the public method on the document renderer to create and cache it.
            page.doc_renderer
                .add_image_xobject(&image_el.src, &image_el.image_data)?
        };

    let y = page.page_height_pt - (positioned.y + positioned.height);
    let transform = XObjectTransform {
        translate_x: Some(Pt(positioned.x)),
        translate_y: Some(Pt(y)),
        scale_x: Some(positioned.width / (img_w as f32)),
        scale_y: Some(positioned.height / (img_h as f32)),
        rotate: None,
        dpi: Some(72.0),
    };
    page.ops.push(Op::UseXobject { id: xobj_id, transform });
    Ok(())
}