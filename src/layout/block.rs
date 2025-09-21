// src/layout/block.rs

//! Layout logic for block-level containers like `Block`, `List`, and `ListItem`.

use super::elements::{LayoutElement, PositionedElement, RectElement, TextElement};
use super::style::ComputedStyle;
use super::{IRNode, WorkItem};
use std::sync::Arc;

/// Lays out a standard block container by pushing its children onto the work stack.
/// The actual height is determined as the children are processed.
pub fn layout_block(
    work_stack: &mut Vec<(WorkItem, Arc<ComputedStyle>)>,
    children: &mut [IRNode],
    style: &Arc<ComputedStyle>,
) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
    work_stack.push((WorkItem::EndNode(style.clone()), style.clone()));
    for child in children.iter().rev() {
        work_stack.push((WorkItem::Node(child.clone()), style.clone()));
    }
    (vec![], 0.0, None)
}

/// Lays out a list item, adding a bullet point and indenting the children.
pub fn layout_list_item(
    work_stack: &mut Vec<(WorkItem, Arc<ComputedStyle>)>,
    children: &mut [IRNode],
    style: &Arc<ComputedStyle>,
) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
    // Place bullet at the start of the content box.
    // The list item itself should have a margin/padding from the XSLT to indent the whole thing.
    let bullet_width = style.font_size * 0.6; // "•" approx
    let bullet_spacing = style.font_size * 0.4;
    let bullet = PositionedElement {
        x: 0.0, // Start of the content box.
        y: style.padding.top, // Align with first line of text
        width: bullet_width,
        height: style.line_height,
        element: LayoutElement::Text(TextElement {
            content: "•".to_string(),
            href: None,
        }),
        style: style.clone(),
    };

    work_stack.push((WorkItem::EndNode(style.clone()), style.clone()));
    let mut indented_style_arc = style.clone();
    let indented_style_mut = Arc::make_mut(&mut indented_style_arc);
    indented_style_mut.padding.left += bullet_width + bullet_spacing;

    for child in children.iter().rev() {
        work_stack.push((WorkItem::Node(child.clone()), indented_style_arc.clone()));
    }

    (vec![bullet], 0.0, None)
}


/// Prepends a background rectangle to a list of elements.
pub fn add_background(
    elements: &mut Vec<PositionedElement>,
    style: &Arc<ComputedStyle>,
    width: f32,
    height: f32,
) {
    elements.insert(
        0,
        PositionedElement {
            x: style.padding.left,
            y: style.padding.top,
            width: width - style.padding.left - style.padding.right,
            height: height - style.padding.top - style.padding.bottom,
            element: LayoutElement::Rectangle(RectElement),
            style: style.clone(),
        },
    );
}
