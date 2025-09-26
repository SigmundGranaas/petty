// src/layout/subtree.rs

//! Non-paginated, recursive layout and measurement for subtrees.
//! Used for elements that are measured and laid out as a single unit within a
//! larger container, such as flexbox items or table cells.

use super::style::ComputedStyle;
use super::{block, flex, image, table, text, IRNode, LayoutEngine, PositionedElement};
use std::sync::Arc;

/// Lays out a node and all its children recursively, assuming it all fits in one block.
/// This is a simplified, non-paginating layout function that dispatches to the
/// appropriate node-specific implementation.
pub(super) fn layout_subtree(
    engine: &LayoutEngine,
    node: &mut IRNode,
    parent_style: &Arc<ComputedStyle>,
    available_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let style = engine.compute_style(node.style_sets(), node.style_override(), parent_style);
    let mut elements = Vec::new();
    let mut total_height = style.margin.top;

    let content_width = available_width - style.padding.left - style.padding.right;

    let (content_elements, content_height) = match node {
        IRNode::Paragraph { .. } => {
            text::layout_paragraph_subtree(engine, node, &style, content_width)
        }
        IRNode::Root(_) | IRNode::Block { .. } | IRNode::List { .. } => {
            block::layout_block_subtree(engine, node, &style, content_width)
        }
        IRNode::ListItem { .. } => {
            block::layout_list_item_subtree(engine, node, &style, content_width)
        }
        IRNode::FlexContainer { .. } => {
            flex::layout_flex_subtree(engine, node, &style, content_width)
        }
        IRNode::Image { .. } => image::layout_image_subtree(node, &style, content_width),
        IRNode::Table { .. } => table::layout_table_subtree(engine, node, &style, content_width),
    };

    // Position the generated content elements within the current node's padding box.
    for mut el in content_elements {
        el.y += total_height;
        // Flexbox layout already handles the children's x positions relative to the container's
        // content box. Other block types do not, so their children need to be indented by padding.
        if !matches!(node, IRNode::FlexContainer { .. }) {
            el.x += style.padding.left;
        }
        elements.push(el);
    }
    total_height += content_height + style.margin.bottom;

    (elements, total_height)
}

/// A cheap, measurement-only version of `layout_subtree`.
/// It calculates the height of a node and its children without allocating any `PositionedElement`s,
/// dispatching to the appropriate node-specific implementation.
pub(super) fn measure_subtree_height(
    engine: &LayoutEngine,
    node: &mut IRNode,
    parent_style: &Arc<ComputedStyle>,
    available_width: f32,
) -> f32 {
    let style = engine.compute_style(node.style_sets(), node.style_override(), parent_style);
    let mut total_height = style.margin.top;

    let content_width = available_width - style.padding.left - style.padding.right;

    let content_height = match node {
        IRNode::Paragraph { .. } => {
            text::measure_paragraph_subtree(engine, node, &style, content_width)
        }
        IRNode::Root(_) | IRNode::Block { .. } | IRNode::List { .. } => {
            block::measure_block_subtree(engine, node, &style, content_width)
        }
        IRNode::ListItem { .. } => {
            block::measure_list_item_subtree(engine, node, &style, content_width)
        }
        IRNode::FlexContainer { .. } => {
            flex::measure_flex_subtree(engine, node, &style, content_width)
        }
        IRNode::Image { .. } => image::measure_image_subtree(node, &style, content_width),
        IRNode::Table { .. } => table::measure_table_subtree(engine, node, &style, available_width),
    };

    total_height += content_height + style.margin.bottom;
    total_height
}