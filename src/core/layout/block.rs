// FILE: src/layout/block.rs
//! Layout logic for block-level containers like `Block`, `List`, and `ListItem`.

use super::elements::{LayoutElement, PositionedElement, RectElement, TextElement};
use super::style::ComputedStyle;
use super::subtree;
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

// --- Subtree Layout ---

/// Lays out a block-like node (Block, List, Root) for a subtree measurement.
pub(super) fn layout_block_subtree(
    engine: &super::LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let children = match node {
        IRNode::Root(children)
        | IRNode::Block { children, .. }
        | IRNode::List { children, .. } => children,
        _ => return (vec![], 0.0),
    };

    let mut block_elements = Vec::with_capacity(children.len() * 4);
    let mut current_y = style.padding.top;
    for child in children {
        let (mut child_elements, child_height) =
            subtree::layout_subtree(engine, child, style, content_width);
        for el in &mut child_elements {
            el.y += current_y;
        }
        block_elements.extend(child_elements);
        current_y += child_height;
    }
    (block_elements, current_y + style.padding.bottom)
}

/// Measures a block-like node for a subtree measurement.
pub(super) fn measure_block_subtree(
    engine: &super::LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> f32 {
    let children = match node {
        IRNode::Root(children)
        | IRNode::Block { children, .. }
        | IRNode::List { children, .. } => children,
        _ => return 0.0,
    };

    let mut current_y = style.padding.top;
    for child in children {
        let child_height = subtree::measure_subtree_height(engine, child, style, content_width);
        current_y += child_height;
    }
    current_y + style.padding.bottom
}

/// Lays out a list item for a subtree measurement.
pub(super) fn layout_list_item_subtree(
    engine: &super::LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let children = match node {
        IRNode::ListItem { children, .. } => children,
        _ => return (vec![], 0.0),
    };

    let bullet_width = style.font_size * 0.6;
    let bullet_spacing = style.font_size * 0.4;
    let bullet = PositionedElement {
        x: 0.0,
        y: style.padding.top,
        width: bullet_width,
        height: style.line_height,
        element: LayoutElement::Text(TextElement {
            content: "•".to_string(),
            href: None,
        }),
        style: style.clone(),
    };

    let mut all_elements = Vec::with_capacity(1 + children.len() * 4);
    all_elements.push(bullet);

    let mut current_y = style.padding.top;
    let indent = bullet_width + bullet_spacing;
    let indented_content_width = content_width - indent;

    for child in children {
        // Lay out the child in the narrower, indented space
        let (mut child_elements, child_height) =
            subtree::layout_subtree(engine, child, style, indented_content_width);

        for el in &mut child_elements {
            el.x += indent; // Apply the indentation
            el.y += current_y;
        }
        all_elements.extend(child_elements);
        current_y += child_height;
    }

    let children_content_height = current_y - style.padding.top;
    let final_content_height = children_content_height.max(style.line_height);

    (
        all_elements,
        final_content_height + style.padding.top + style.padding.bottom,
    )
}

/// Measures a list item for a subtree measurement.
pub(super) fn measure_list_item_subtree(
    engine: &super::LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> f32 {
    let children = match node {
        IRNode::ListItem { children, .. } => children,
        _ => return 0.0,
    };

    let bullet_width = style.font_size * 0.6;
    let bullet_spacing = style.font_size * 0.4;
    let indent = bullet_width + bullet_spacing;
    let indented_content_width = content_width - indent;
    let mut current_y = style.padding.top;

    for child in children {
        let child_height =
            subtree::measure_subtree_height(engine, child, style, indented_content_width);
        current_y += child_height;
    }

    let children_content_height = current_y - style.padding.top;
    let final_content_height = children_content_height.max(style.line_height);

    final_content_height + style.padding.top + style.padding.bottom
}