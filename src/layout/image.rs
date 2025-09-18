// src/layout/image.rs

//! Layout logic for image elements.

use super::elements::{ImageElement, LayoutElement, PositionedElement};
use super::style::ComputedStyle;
use crate::stylesheet::Dimension;
use std::sync::Arc;

pub fn layout_image(
    src: &str,
    data: Option<&Arc<Vec<u8>>>,
    style: &ComputedStyle,
    available_width: f32,
) -> (Vec<PositionedElement>, f32, Option<super::WorkItem>) {
    let content_width = available_width - style.padding.left - style.padding.right;

    let height = style.height.unwrap_or(50.0);
    let width = match style.width {
        Some(Dimension::Pt(w)) => w,
        Some(Dimension::Percent(p)) => content_width * (p / 100.0),
        _ => content_width,
    };

    if let Some(image_data) = data {
        let el = PositionedElement {
            x: 0.0,
            y: 0.0,
            width,
            height,
            element: LayoutElement::Image(ImageElement {
                src: src.to_string(),
                image_data: image_data.clone(),
            }),
            style: style.clone(),
        };
        (vec![el], height, None)
    } else {
        log::warn!("Image data not found for src: {}", src);
        (vec![], 0.0, None)
    }
}