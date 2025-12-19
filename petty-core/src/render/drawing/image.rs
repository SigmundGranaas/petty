#![allow(dead_code)]

use super::super::pdf::{PageRenderState, PageRenderer, RenderContext};
use crate::render::RenderError;
use printpdf::ops::Op;
use printpdf::xobject::XObjectTransform;
use printpdf::Pt;
use std::io;
use crate::core::layout::{ImageElement, PositionedElement};

/// Renders an `ImageElement` to the page.
pub(super) fn draw_image<W: io::Write + Send>(
    page: &mut PageRenderer<W>,
    image_el: &ImageElement,
    positioned: &PositionedElement,
) -> Result<(), RenderError> {
    // Images cannot be drawn within a text section.
    if page.state.is_text_section_open {
        page.ops.push(Op::EndTextSection);
        page.state.is_text_section_open = false;
    }

    // The orchestrator pre-loads all resources for a sequence into the renderer's
    // XObject cache. Here, we just look up the cached object.
    let (xobj_id, (img_w, img_h)) =
        if let Some(cached) = page.doc_renderer.image_xobjects.get(&image_el.src) {
            (cached.0.clone(), cached.1)
        } else {
            // This case signifies that a resource was in the layout tree but failed to load
            // or was not passed to the renderer correctly. We'll skip drawing it.
            log::warn!(
                "Image resource not found in document cache, skipping render: {}",
                image_el.src
            );
            return Ok(());
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
    page.ops
        .push(Op::UseXobject { id: xobj_id, transform });
    Ok(())
}

/// Renders an `ImageElement` statelessly by looking up the XObject ID from the context.
pub(super) fn draw_image_stateless(
    ops: &mut Vec<Op>,
    state: &mut PageRenderState,
    ctx: &RenderContext,
    image_el: &ImageElement,
    positioned: &PositionedElement,
) -> Result<(), RenderError> {
    if state.is_text_section_open {
        ops.push(Op::EndTextSection);
        state.is_text_section_open = false;
    }

    // Look up the pre-cached XObject from the context.
    let (xobj_id, (img_w, img_h)) =
        ctx.image_xobjects.get(&image_el.src).ok_or_else(|| {
            RenderError::Other(format!("Image not found in cache: {}", image_el.src))
        })?;

    let y = ctx.page_height_pt - (positioned.y + positioned.height);
    let transform = XObjectTransform {
        translate_x: Some(Pt(positioned.x)),
        translate_y: Some(Pt(y)),
        scale_x: Some(positioned.width / (*img_w as f32)),
        scale_y: Some(positioned.height / (*img_h as f32)),
        rotate: None,
        dpi: Some(72.0),
    };
    ops.push(Op::UseXobject {
        id: xobj_id.clone(),
        transform,
    });
    Ok(())
}