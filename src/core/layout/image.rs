// src/layout/image.rs
use super::elements::{ImageElement, LayoutElement, PositionedElement};
use super::style::ComputedStyle;
use std::sync::Arc;
use crate::core::idf::IRNode;
use crate::core::style::dimension::Dimension;

pub fn layout_image(
    src: &str,
    style: &Arc<ComputedStyle>,
    available_width: f32,
) -> (Vec<PositionedElement>, f32, Option<super::WorkItem>) {
    let content_width = available_width - style.padding.left - style.padding.right;

    let (width, height) = calculate_image_dimensions(style, content_width);

    if src.is_empty() {
        log::warn!("Image with empty src found, rendering nothing.");
        return (vec![], 0.0, None);
    }

    let el = PositionedElement {
        x: 0.0,
        y: 0.0,
        width,
        height,
        element: LayoutElement::Image(ImageElement {
            src: src.to_string(),
        }),
        style: style.clone(),
    };
    // An image is an atomic unit; it consumes its height and has no pending work.
    (vec![el], height, None)
}

// --- Subtree Layout ---

/// Lays out an image for a subtree measurement.
pub(super) fn layout_image_subtree(
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let src = if let IRNode::Image { src, .. } = node {
        src
    } else {
        return (vec![], 0.0);
    };
    let (els, height, _remainder) = layout_image(src, style, content_width);
    (els, height + style.padding.top + style.padding.bottom)
}

/// Measures an image for a subtree measurement.
pub(super) fn measure_image_subtree(
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> f32 {
    let src = if let IRNode::Image { src, .. } = node {
        src
    } else {
        return 0.0;
    };
    let (_, height) = calculate_image_dimensions(style, content_width);

    if src.is_empty() {
        0.0
    } else {
        height + style.padding.top + style.padding.bottom
    }
}

/// Helper to resolve image dimensions from style and available width.
fn calculate_image_dimensions(style: &Arc<ComputedStyle>, content_width: f32) -> (f32, f32) {
    // The `ComputedStyle` resolves height to a concrete point value or None.
    // If no height is specified, we use a default. A future implementation could
    // use the image's intrinsic size.
    let height = style.height.unwrap_or(50.0);

    // The width can be a point, percentage, or auto, so we resolve it here based on
    // the available width in the parent container.
    let width = match &style.width {
        Some(Dimension::Pt(w)) => *w,
        Some(Dimension::Percent(p)) => content_width * (p / 100.0),
        // Default for Auto, Px, or None is to fill the available content width.
        _ => content_width,
    };
    (width, height)
}