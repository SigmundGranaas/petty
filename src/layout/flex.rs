// src/layout/flex.rs

//! Layout logic for flexbox containers.

use super::style::ComputedStyle;
use super::{IRNode, LayoutEngine, PositionedElement};
use crate::stylesheet::Dimension;

/// Lays out a flex container's children horizontally.
/// Note: This is a simplified implementation and does not handle wrapping or complex flex properties.
pub fn layout_flex_container(
    engine: &LayoutEngine,
    children: &mut [IRNode],
    style: &ComputedStyle,
    available_width: f32,
) -> (Vec<PositionedElement>, f32, Option<super::WorkItem>) {
    let child_widths = calculate_flex_child_widths(engine, children, available_width);
    let mut all_elements = Vec::new();
    let mut max_height = 0.0f32;
    let mut current_x = 0.0;
    let mut child_layouts = Vec::new();

    // First pass: lay out all children to determine their height.
    for (i, child) in children.iter_mut().enumerate() {
        let child_width = child_widths[i];
        // Temporarily lay out the subtree to measure its height.
        let (child_elements, child_height) = layout_subtree(engine, child, style, child_width);
        max_height = max_height.max(child_height);
        child_layouts.push((child_elements, child_width));
    }

    // Second pass: position the elements horizontally.
    for (mut child_elements, child_width) in child_layouts {
        for el in &mut child_elements {
            el.x += current_x;
        }
        all_elements.extend(child_elements);
        current_x += child_width;
    }
    (all_elements, max_height, None)
}

/// Calculates the widths of children inside a flex container.
fn calculate_flex_child_widths(
    engine: &LayoutEngine,
    children: &[IRNode],
    available_width: f32,
) -> Vec<f32> {
    if children.is_empty() {
        return vec![];
    }
    let mut widths = vec![0.0; children.len()];
    let mut auto_indices = Vec::new();
    let mut remaining_width = available_width;

    let default_parent_style = engine.get_default_style();
    for (i, child) in children.iter().enumerate() {
        let style = engine.compute_style(child.style_name(), &default_parent_style);
        if let Some(dim) = &style.width {
            match dim {
                Dimension::Pt(w) => {
                    widths[i] = *w;
                    remaining_width -= *w;
                }
                Dimension::Percent(p) => {
                    widths[i] = (p / 100.0) * available_width;
                    remaining_width -= widths[i];
                }
                _ => auto_indices.push(i),
            }
        } else {
            auto_indices.push(i);
        }
    }

    if !auto_indices.is_empty() && remaining_width > 0.0 {
        let width_per_auto = remaining_width / auto_indices.len() as f32;
        for i in auto_indices {
            widths[i] = width_per_auto;
        }
    }
    widths
}


/// Lays out a node and all its children recursively, assuming it all fits in one block.
/// This is a simplified, non-paginating layout function used for measuring items
/// within containers like flexbox or table cells.
pub(super) fn layout_subtree(
    engine: &LayoutEngine,
    node: &mut IRNode,
    parent_style: &ComputedStyle,
    available_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let style = engine.compute_style(node.style_name(), parent_style);
    let mut elements = Vec::new();
    let mut total_height = style.margin.top;

    let content_width = available_width - style.padding.left - style.padding.right;

    let (content_elements, content_height) = match node {
        IRNode::Paragraph { children, .. } => {
            // Since this is for measurement, we assume infinite vertical space.
            // We ignore the `remainder` Option from layout_paragraph's return.
            let (els, height, _remainder) =
                super::text::layout_paragraph(engine, children, &style, content_width, f32::MAX);
            (els, height + style.padding.top + style.padding.bottom)
        }
        IRNode::Root(children) | IRNode::Block { children, .. } => {
            let mut block_elements = Vec::new();
            let mut current_y = style.padding.top;
            for child in children {
                let (mut child_elements, child_height) =
                    layout_subtree(engine, child, &style, content_width);
                for el in &mut child_elements {
                    el.y += current_y;
                }
                block_elements.extend(child_elements);
                current_y += child_height;
            }
            (block_elements, current_y + style.padding.bottom)
        }
        _ => (vec![], 0.0), // Simplified for brevity
    };

    for mut el in content_elements {
        el.y += total_height;
        el.x += style.padding.left;
        elements.push(el);
    }
    total_height += content_height + style.margin.bottom;

    (elements, total_height)
}