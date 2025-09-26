use super::style::ComputedStyle;
use super::{subtree, IRNode, LayoutEngine, PositionedElement, WorkItem};
use std::sync::Arc;
use crate::core::style::dimension::Dimension;

/// Lays out a flex container's children horizontally by pushing control items
/// and the children themselves onto the main work stack for the paginated layout engine.
pub fn layout_flex_container(
    work_stack: &mut Vec<(WorkItem, Arc<ComputedStyle>)>,
    children: &mut [IRNode],
    style: &Arc<ComputedStyle>,
    engine: &LayoutEngine,
    available_width: f32,
) {
    // 1. Pre-calculate the widths of all children. This is the "measurement" part.
    let child_widths = calculate_flex_child_widths(engine, children, available_width, style);

    // 2. Push markers and children onto the main work stack to be handled by the PageIterator.
    // The PageIterator will manage the horizontal layout based on these markers.

    // Push EndNode first to handle the container's own bottom margin/padding.
    work_stack.push((WorkItem::EndNode(style.clone()), style.clone()));
    // Push EndFlex to calculate the container's content height after children are laid out.
    work_stack.push((WorkItem::EndFlex, style.clone()));

    // Push children in reverse order so they are processed from left to right.
    for child in children.iter().rev() {
        work_stack.push((WorkItem::Node(child.clone()), style.clone()));
    }

    // Push the start marker with the calculated widths.
    work_stack.push((WorkItem::StartFlex(child_widths), style.clone()));
}

// --- Subtree Layout ---

/// Lays out a flex container for a subtree measurement (non-paginated context).
pub(super) fn layout_flex_subtree(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let children = match node {
        IRNode::FlexContainer { children, .. } => children,
        _ => return (vec![], 0.0),
    };
    let (els, height) = layout_flex_children_subtree(engine, children, style, content_width);
    (els, height + style.padding.top + style.padding.bottom)
}

/// Measures a flex container for a subtree measurement.
pub(super) fn measure_flex_subtree(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> f32 {
    let children = match node {
        IRNode::FlexContainer { children, .. } => children,
        _ => return 0.0,
    };
    let height = measure_flex_children_subtree(engine, children, style, content_width);
    height + style.padding.top + style.padding.bottom
}

/// Lays out flex children in a single pass, used for non-paginated contexts.
fn layout_flex_children_subtree(
    engine: &LayoutEngine,
    children: &mut [IRNode],
    parent_style: &Arc<ComputedStyle>,
    available_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let child_widths = calculate_flex_child_widths(engine, children, available_width, parent_style);
    let mut all_elements = Vec::with_capacity(children.len() * 4);
    let mut max_child_height = 0.0f32;
    let mut child_layout_results = Vec::with_capacity(children.len());

    for (i, child) in children.iter_mut().enumerate() {
        let child_width = child_widths[i];
        let (child_elements, child_height) =
            subtree::layout_subtree(engine, child, parent_style, child_width);
        max_child_height = max_child_height.max(child_height);
        child_layout_results.push(child_elements);
    }

    let mut current_x = 0.0;
    for (i, mut child_elements) in child_layout_results.into_iter().enumerate() {
        let child_width = child_widths[i];
        for el in &mut child_elements {
            el.x += current_x;
        }
        all_elements.extend(child_elements);
        current_x += child_width;
    }

    (all_elements, max_child_height)
}

/// Measures flex children for non-paginated contexts.
fn measure_flex_children_subtree(
    engine: &LayoutEngine,
    children: &mut [IRNode],
    parent_style: &Arc<ComputedStyle>,
    available_width: f32,
) -> f32 {
    let child_widths = calculate_flex_child_widths(engine, children, available_width, parent_style);
    let mut max_child_height = 0.0f32;

    for (i, child) in children.iter_mut().enumerate() {
        let child_width = child_widths[i];
        let child_height = subtree::measure_subtree_height(engine, child, parent_style, child_width);
        max_child_height = max_child_height.max(child_height);
    }
    max_child_height
}

/// Calculates the widths of children inside a flex container.
fn calculate_flex_child_widths(
    engine: &LayoutEngine,
    children: &[IRNode],
    available_width: f32,
    parent_style: &Arc<ComputedStyle>,
) -> Vec<f32> {
    if children.is_empty() {
        return vec![];
    }
    let mut widths = vec![0.0; children.len()];
    let mut auto_indices = Vec::with_capacity(children.len());
    let mut remaining_width = available_width;

    for (i, child) in children.iter().enumerate() {
        let style = engine.compute_style(
            child.style_sets(),
            child.style_override(),
            parent_style,
        );
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