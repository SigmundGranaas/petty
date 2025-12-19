use crate::elements::RectElement;
use crate::style::{ComputedStyle, ComputedStyleData};
use crate::{LayoutElement, PositionedElement};
use petty_style::border::Border;
use petty_types::geometry::Rect;
use std::sync::Arc;

/// Generates background and border elements for a rectangular region.
///
/// This function is stateless and pure, depending only on the provided arguments.
/// It is used by Block, Flex, Table Cell, and List Item nodes.
pub fn create_background_and_borders(
    bounds: Rect,
    style: &ComputedStyle,
    start_y: f32,
    content_height: f32,
    draw_top: bool,
    draw_bottom: bool,
) -> Vec<PositionedElement> {
    let mut elements = Vec::new();

    let border_top = if draw_top {
        style.border_top_width()
    } else {
        0.0
    };
    let border_bottom = if draw_bottom {
        style.border_bottom_width()
    } else {
        0.0
    };
    let border_left = style.border_left_width();
    let border_right = style.border_right_width();

    let padding_top = if draw_top {
        style.box_model.padding.top
    } else {
        0.0
    };
    let padding_bottom = if draw_bottom {
        style.box_model.padding.bottom
    } else {
        0.0
    };

    let total_height = border_top + padding_top + content_height + padding_bottom + border_bottom;

    if total_height <= 0.0 {
        return elements;
    }

    // Helper closure to push elements relative to the node's start position
    let mut push = |mut element: PositionedElement, x: f32, y: f32| {
        element.x += x;
        element.y += y;
        elements.push(element);
    };

    // 1. Draw Background
    if style.misc.background_color.is_some() {
        let mut bg_data = ComputedStyleData::default();
        bg_data.misc.background_color = style.misc.background_color.clone();
        let bg_style = ComputedStyle::new(bg_data);

        // Background is drawn inside borders
        let bg_rect = Rect {
            x: border_left,
            y: border_top,
            width: bounds.width - border_left - border_right,
            height: total_height - border_top - border_bottom,
        };
        let bg = PositionedElement {
            element: LayoutElement::Rectangle(RectElement),
            style: Arc::new(bg_style),
            ..PositionedElement::from_rect(bg_rect)
        };
        push(bg, 0.0, start_y);
    }

    // 2. Draw Borders
    let bounds_width = bounds.width;

    let mut draw_border = |b: &Option<Border>, rect: Rect| {
        #[allow(clippy::collapsible_if)]
        if let Some(border) = b {
            if border.width > 0.0 {
                let mut border_data = ComputedStyleData::default();
                border_data.misc.background_color = Some(border.color.clone());
                let border_style = ComputedStyle::new(border_data);

                let positioned_rect = PositionedElement {
                    element: LayoutElement::Rectangle(RectElement),
                    style: Arc::new(border_style),
                    ..PositionedElement::from_rect(rect)
                };
                push(positioned_rect, 0.0, start_y);
            }
        }
    };

    if draw_top {
        draw_border(
            &style.border.top,
            Rect {
                x: 0.0,
                y: 0.0,
                width: bounds_width,
                height: border_top,
            },
        );
    }
    if draw_bottom {
        draw_border(
            &style.border.bottom,
            Rect {
                x: 0.0,
                y: total_height - border_bottom,
                width: bounds_width,
                height: border_bottom,
            },
        );
    }

    draw_border(
        &style.border.left,
        Rect {
            x: 0.0,
            y: 0.0,
            width: border_left,
            height: total_height,
        },
    );
    draw_border(
        &style.border.right,
        Rect {
            x: bounds_width - border_right,
            y: 0.0,
            width: border_right,
            height: total_height,
        },
    );

    elements
}
