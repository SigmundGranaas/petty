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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::style::ComputedStyle;

    #[test]
    fn test_layout_empty_src_image() {
        let style = Arc::new(ComputedStyle::default());
        let (elements, height, _pending) = layout_image("", &style, 500.0);

        assert!(elements.is_empty(), "No elements should be produced for an empty src");
        assert_eq!(height, 0.0, "Height should be zero for an empty src");
    }

    #[test]
    fn test_calculate_image_dimensions() {
        let container_width = 200.0;

        // Case 1: No width or height specified (defaults)
        let style1 = ComputedStyle::default();
        let (w1, h1) = calculate_image_dimensions(&Arc::new(style1), container_width);
        assert_eq!(w1, container_width); // Defaults to fill container
        assert_eq!(h1, 50.0); // Defaults to 50pt height

        // Case 2: Fixed point width and height
        let style2 = ComputedStyle {
            width: Some(Dimension::Pt(80.0)),
            height: Some(60.0),
            ..ComputedStyle::default()
        };
        let (w2, h2) = calculate_image_dimensions(&Arc::new(style2), container_width);
        assert_eq!(w2, 80.0);
        assert_eq!(h2, 60.0);

        // Case 3: Percentage width
        let style3 = ComputedStyle {
            width: Some(Dimension::Percent(75.0)), // 75% of 200 = 150
            ..ComputedStyle::default()
        };
        let (w3, h3) = calculate_image_dimensions(&Arc::new(style3), container_width);
        assert_eq!(w3, 150.0);
        assert_eq!(h3, 50.0); // Default height
    }
}