// FILE: src/layout/page.rs
//! The stateful page iterator for the positioning pass.

use super::block;
use super::elements::PositionedElement;
use super::flex;
use super::image;
use super::style;
use super::subtree;
use super::table;
use super::text;
use super::{IRNode, LayoutEngine, WorkItem};
use std::sync::Arc;
use crate::core::idf::LayoutUnit;
use crate::core::style::dimension::{Dimension, Margins};

/// A stateful iterator that performs the **Positioning Pass** of the layout algorithm.
/// It consumes a pre-measured `IRNode` tree and yields pages of positioned elements.
pub struct PageIterator<'a> {
    engine: &'a LayoutEngine,
    work_stack: Vec<(WorkItem, Arc<style::ComputedStyle>)>,
    current_y: f32,
    page_width: f32,
    page_height: f32,
    content_bottom: f32,
    margins: &'a Margins,
    is_finished: bool,
    flex_stack: Vec<FlexContext>,
    block_stack: Vec<BlockContext>,
}

/// State for an active flex container being laid out.
#[derive(Clone)]
struct FlexContext {
    child_widths: Vec<f32>,
    child_heights: Vec<f32>,
    next_child_index: usize,
    start_y: f32,
}

#[derive(Clone)]
struct BlockContext {
    /// The absolute horizontal starting position for content in this block.
    x_offset: f32,
    /// The available width for content inside this block.
    content_width: f32,
    /// The style of the node that created this context, needed for popping.
    style: Arc<style::ComputedStyle>,
}

impl<'a> PageIterator<'a> {
    pub fn new(layout_unit: LayoutUnit, engine: &'a LayoutEngine) -> Self {
        let (page_width, page_height) = style::get_page_dimensions(&engine.stylesheet);
        let margins = &engine.stylesheet.page.margins;
        let content_bottom = margins.bottom + engine.stylesheet.page.footer_height;

        let mut work_stack = Vec::new();
        let default_style = engine.get_default_style();

        if let IRNode::Root(children) = layout_unit.tree {
            work_stack.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|node| (WorkItem::Node(node), default_style.clone())),
            );
        }

        let root_context = BlockContext {
            x_offset: margins.left,
            content_width: page_width - margins.left - margins.right,
            style: default_style.clone(),
        };

        Self {
            engine,
            work_stack,
            current_y: margins.top,
            page_width,
            page_height,
            content_bottom,
            margins,
            is_finished: false,
            flex_stack: Vec::new(),
            block_stack: vec![root_context],
        }
    }

    fn layout_node(
        &mut self,
        node: &mut IRNode,
        style: &Arc<style::ComputedStyle>,
        available_width: f32,
        available_height: f32,
    ) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
        let margin_top = style.margin.top;

        if margin_top >= available_height {
            return (vec![], 0.0, Some(WorkItem::Node(node.clone())));
        }

        let inner_available_height = available_height - margin_top;

        let node_own_width = match &style.width {
            Some(Dimension::Pt(w)) => *w,
            Some(Dimension::Percent(p)) => available_width * (p / 100.0),
            _ => available_width,
        };
        let child_content_width = node_own_width - style.padding.left - style.padding.right;

        if child_content_width <= 1.0 && !matches!(node, IRNode::Image { .. }) {
            log::warn!(
                "Node content width is near-zero or negative ({:.2}pt) due to padding/width constraints. Content may be invisible. Node type: {:?}",
                child_content_width,
                std::mem::discriminant(node)
            );
        }

        let (mut elements, mut content_height, pending_content) = match node {
            IRNode::Paragraph { children, style_sets, style_override } => {
                text::layout_paragraph_node(
                    self.engine,
                    children,
                    style_sets,
                    style_override,
                    style,
                    child_content_width,
                    inner_available_height,
                )
            }
            IRNode::Block { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => {
                let parent_ctx = self.block_stack.last().unwrap();
                let new_x_offset = parent_ctx.x_offset + style.margin.left + style.padding.left;

                let new_content_width = style.width.as_ref().map_or_else(
                    || parent_ctx.content_width - style.margin.left - style.margin.right,
                    |dim| match dim {
                        Dimension::Pt(w) => *w,
                        Dimension::Percent(p) => parent_ctx.content_width * (p / 100.0),
                        _ => parent_ctx.content_width - style.margin.left - style.margin.right,
                    },
                );

                self.block_stack.push(BlockContext {
                    x_offset: new_x_offset,
                    content_width: new_content_width - style.padding.left - style.padding.right,
                    style: style.clone(),
                });

                self.work_stack.push((WorkItem::EndNode(style.clone()), style.clone()));
                for child in children.iter().rev() {
                    self.work_stack.push((WorkItem::Node(child.clone()), style.clone()));
                }

                let consumed_height = style.margin.top + style.padding.top;
                return (vec![], consumed_height, None);
            }
            IRNode::FlexContainer { children, .. } => {
                flex::layout_flex_container(
                    &mut self.work_stack,
                    children,
                    style,
                    self.engine,
                    child_content_width,
                );
                let consumed_height = style.margin.top + style.padding.top;
                return (vec![], consumed_height, None);
            }
            IRNode::Image { src, .. } => image::layout_image(src, style, child_content_width),
            IRNode::Table { .. } => {
                table::layout_table_node(self.engine, node, style, inner_available_height)
            }
            IRNode::Root(_) => (vec![], 0.0, None),
        };

        if let Some(h) = style.height {
            content_height = content_height.max(h);
        }

        for el in &mut elements {
            el.x += style.padding.left;
            el.y += style.padding.top;
        }

        let height_with_padding = content_height + style.padding.top + style.padding.bottom;
        let total_node_height = margin_top + height_with_padding + style.margin.bottom;

        if pending_content.is_none() {
            if total_node_height > available_height {
                let fresh_page_content_height =
                    self.page_height - self.margins.top - self.content_bottom;
                if total_node_height > fresh_page_content_height {
                    log::error!(
                        "Node has a height of {:.2} which exceeds the total page content height of {:.2}. The node will be skipped.",
                        total_node_height, fresh_page_content_height
                    );
                    return (vec![], 0.0, None);
                } else {
                    return (vec![], 0.0, Some(WorkItem::Node(node.clone())));
                }
            }
        }

        if style.background_color.is_some() || style.border_bottom.is_some() {
            block::add_background(&mut elements, style, node_own_width, height_with_padding);
        }

        for el in &mut elements {
            el.y += margin_top;
        }

        let height_consumed_on_page = margin_top + height_with_padding;

        (elements, height_consumed_on_page, pending_content)
    }
}

impl<'a> Iterator for PageIterator<'a> {
    type Item = Vec<PositionedElement>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_finished {
            return None;
        }

        let mut page_elements = Vec::new();
        self.current_y = self.margins.top;

        let mut work_processed = false;

        while let Some((work_item, parent_style)) = self.work_stack.pop() {
            work_processed = true;

            let remaining_height = self.page_height - self.content_bottom - self.current_y;
            if remaining_height <= 0.0
                && !matches!(work_item, WorkItem::EndFlex | WorkItem::EndNode(_))
            {
                self.work_stack.push((work_item, parent_style));
                return Some(page_elements);
            }

            let (elements, consumed_height, pending_work) = match work_item {
                WorkItem::Node(mut node) => {
                    let style =
                        self.engine.compute_style(node.style_sets(), node.style_override(), &parent_style);

                    if self.flex_stack.last().is_some() {
                        let flex_ctx = self.flex_stack.pop().unwrap();
                        let child_idx = flex_ctx.next_child_index;
                        let child_width = flex_ctx.child_widths.get(child_idx).cloned().unwrap_or(0.0);
                        let child_start_x = self.block_stack.last().unwrap().x_offset
                            + flex_ctx.child_widths.iter().take(child_idx).sum::<f32>();

                        let item_height = subtree::measure_subtree_height(self.engine, &mut node, &style, child_width);
                        self.flex_stack.push(flex_ctx);

                        if item_height > remaining_height && !page_elements.is_empty() {
                            self.work_stack.push((WorkItem::Node(node), parent_style));
                            return Some(page_elements);
                        }

                        let fresh_page_content_height = self.page_height - self.margins.top - self.content_bottom;
                        if item_height > fresh_page_content_height {
                            log::error!( "Flex item is taller ({:.2}pt) than available page height ({:.2}pt) and will be skipped.", item_height, fresh_page_content_height);
                            if let Some(ctx) = self.flex_stack.last_mut() {
                                ctx.child_heights.push(0.0);
                                ctx.next_child_index += 1;
                            }
                            (vec![], 0.0, None)
                        } else if item_height > remaining_height {
                            self.work_stack.push((WorkItem::Node(node), parent_style));
                            return Some(page_elements);
                        } else {
                            let (mut els, actual_height) = subtree::layout_subtree(self.engine, &mut node, &style, child_width);
                            if let Some(ctx) = self.flex_stack.last_mut() {
                                for el in &mut els {
                                    el.x += child_start_x;
                                    el.y += ctx.start_y;
                                }
                                ctx.child_heights.push(actual_height);
                                ctx.next_child_index += 1;
                            }
                            (els, 0.0, None)
                        }
                    } else {
                        // FIX: Distinguish between container nodes and atomic nodes.
                        let is_container_node = matches!(
                            node,
                            IRNode::Block { .. } | IRNode::List { .. } | IRNode::ListItem { .. }
                        );

                        let (current_content_width, current_x_offset) = {
                            let ctx = self.block_stack.last().unwrap();
                            (ctx.content_width, ctx.x_offset)
                        };

                        let (mut els, consumed, pending) =
                            self.layout_node(&mut node, &style, current_content_width, remaining_height);

                        // Only push an EndNode for atomic nodes that don't manage their own context.
                        if pending.is_none() && !is_container_node {
                            self.work_stack.push((WorkItem::EndNode(style.clone()), parent_style.clone()));
                        }

                        for el in &mut els {
                            el.x += current_x_offset;
                            el.y += self.current_y;
                        }
                        (els, consumed, pending)
                    }
                }
                WorkItem::EndNode(style) => {
                    // This `EndNode` could be from a container OR an atomic node.
                    // Only pop the block_stack if the style of the context matches
                    // the style of the EndNode marker.
                    if let Some(ctx) = self.block_stack.last() {
                        if Arc::ptr_eq(&ctx.style, &style) {
                            let ended_ctx = self.block_stack.pop().unwrap();
                            let bottom_space = ended_ctx.style.margin.bottom + ended_ctx.style.padding.bottom;
                            (vec![], bottom_space, None)
                        } else {
                            // This EndNode is for an atomic node. Just add its bottom margin.
                            (vec![], style.margin.bottom, None)
                        }
                    } else {
                        // Should not be reached, but as a fallback, just use margin.
                        (vec![], style.margin.bottom, None)
                    }
                }
                WorkItem::StartFlex(widths) => {
                    self.flex_stack.push(FlexContext {
                        child_widths: widths,
                        child_heights: Vec::new(),
                        next_child_index: 0,
                        start_y: self.current_y,
                    });
                    (vec![], 0.0, None)
                }
                WorkItem::EndFlex => {
                    if let Some(flex_ctx) = self.flex_stack.pop() {
                        let max_height =
                            flex_ctx.child_heights.iter().cloned().fold(0.0f32, f32::max);
                        (vec![], max_height, None)
                    } else {
                        (vec![], 0.0, None)
                    }
                }
            };

            if let Some(pending) = pending_work {
                self.work_stack.push((pending, parent_style));
                page_elements.extend(elements);
                return Some(page_elements);
            }

            page_elements.extend(elements);
            if self.flex_stack.is_empty() {
                self.current_y += consumed_height;
            }
        }

        self.is_finished = true;

        if work_processed || !page_elements.is_empty() {
            Some(page_elements)
        } else {
            None
        }
    }
}