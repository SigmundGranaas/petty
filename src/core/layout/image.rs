// src/layout/image.rs
use super::style::ComputedStyle;
use super::{IRNode, LayoutBox, LayoutContent, Rect};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

pub fn layout_image(
    node: &IRNode,
    style: Arc<ComputedStyle>,
    available_size: (f32, f32),
) -> LayoutBox {
    let src = if let IRNode::Image { src, .. } = node {
        src
    } else {
        return LayoutBox {
            rect: Rect::default(),
            style,
            content: LayoutContent::Children(vec![]),
        };
    };

    if src.is_empty() {
        log::warn!("Image with empty src found, rendering nothing.");
        return LayoutBox {
            rect: Rect::default(),
            style,
            content: LayoutContent::Children(vec![]),
        };
    }

    // TODO: A proper implementation should parse the image file to get its intrinsic
    // dimensions and aspect ratio, which would then be used for 'auto' width/height.
    let content_width = available_size.0 - style.padding.left - style.padding.right;
    let content_height = available_size.1 - style.padding.top - style.padding.bottom;

    let width = match &style.width {
        Some(Dimension::Pt(w)) => *w,
        Some(Dimension::Percent(p)) => content_width * (p / 100.0),
        _ => content_width,
    };

    let height = match &style.height {
        Some(Dimension::Pt(h)) => *h,
        Some(Dimension::Percent(p)) => content_height * (p / 100.0),
        // If height is auto, assume a square aspect ratio for now.
        _ => width,
    };

    LayoutBox {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        },
        style,
        content: LayoutContent::Image(src.clone()),
    }
}