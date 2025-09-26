// FILE: src/layout/page.rs
//! The stateful page iterator for the positioning pass.

use super::block;
use super::elements::PositionedElement;
use super::flex;
use super::image;
use super::style;
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
}

/// State for an active flex container being laid out.
#[derive(Clone)]
struct FlexContext {
    /// The pre-calculated widths of all children in this container.
    child_widths: Vec<f32>,
    /// The final calculated heights of children that have been laid out.
    child_heights: Vec<f32>,
    /// The index of the *next* child to be processed.
    next_child_index: usize,
    /// The Y coordinate where the flex container's content box starts.
    start_y: f32,
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
        }
    }

    fn content_width(&self) -> f32 {
        self.page_width - self.margins.left - self.margins.right
    }

    fn content_left(&self) -> f32 {
        self.margins.left
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

        // Add diagnostic warning for squished content.
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
            // A container node pushes its children onto the stack and does not produce
            // elements directly. It consumes its top margin/padding here, and the
            // EndNode marker will consume the bottom margin/padding.
            IRNode::Block { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => {
                let (els, _, _) = block::layout_block(&mut self.work_stack, children, style);
                let consumed_height = style.margin.top + style.padding.top;
                return (els, consumed_height, None);
            }
            IRNode::FlexContainer { children, .. } => {
                // This is a container. Like Block, it pushes work to the stack
                // and consumes its top margin/padding. It doesn't produce elements directly.
                flex::layout_flex_container(
                    &mut self.work_stack,
                    children,
                    style,
                    self.engine,
                    child_content_width,
                );
                // Return early, consuming only top margin and padding for now.
                // The EndFlex/EndNode markers will handle the rest.
                let consumed_height = style.margin.top + style.padding.top;
                return (vec![], consumed_height, None);
            }
            IRNode::Image { src, .. } => image::layout_image(src, style, child_content_width),
            IRNode::Table { .. } => {
                table::layout_table_node(self.engine, node, style, inner_available_height)
            }
            IRNode::Root(_) => (vec![], 0.0, None), // Root is handled by the iterator constructor
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
            // For atomic nodes (not paginated), check if they fit.
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
                    let style = self
                        .engine
                        .compute_style(node.style_sets(), node.style_override(), &parent_style);

                    // A node that pushes its own end marker (like containers) doesn't need one added after.
                    let pushes_own_end_node = matches!(
                        node,
                        IRNode::FlexContainer { .. }
                            | IRNode::Block { .. }
                            | IRNode::List { .. }
                            | IRNode::ListItem { .. }
                    );

                    let (x_offset, available_width) =
                        if let Some(flex_ctx) = self.flex_stack.last() {
                            let child_idx = flex_ctx.next_child_index;
                            let child_width =
                                flex_ctx.child_widths.get(child_idx).cloned().unwrap_or(0.0);
                            let child_start_x = self.content_left()
                                + flex_ctx.child_widths.iter().take(child_idx).sum::<f32>();
                            (child_start_x, child_width)
                        } else {
                            (self.content_left(), self.content_width())
                        };

                    let (mut els, consumed, pending) = self.layout_node(
                        &mut node,
                        &style,
                        available_width,
                        remaining_height,
                    );

                    // If the node was atomic (no pending work) and doesn't manage its own end marker,
                    // we need to push one to account for its bottom margin.
                    if pending.is_none() && !pushes_own_end_node {
                        self.work_stack
                            .push((WorkItem::EndNode(style.clone()), parent_style.clone()));
                    }

                    if let Some(flex_ctx) = self.flex_stack.last_mut() {
                        // All direct children of a flex container are flex items.
                        flex_ctx.child_heights.push(consumed);
                        flex_ctx.next_child_index += 1;
                        // Position elements relative to the flex container's start.
                        for el in &mut els {
                            el.x += x_offset;
                            el.y += flex_ctx.start_y;
                        }
                        (els, 0.0, pending) // Height is handled by EndFlex
                    } else {
                        // Position elements relative to the current page flow.
                        for el in &mut els {
                            el.x += x_offset;
                            el.y += self.current_y;
                        }
                        (els, consumed, pending)
                    }
                }
                WorkItem::EndNode(style) => {
                    // This now handles bottom margin AND padding for containers. For atomic nodes,
                    // their padding was already included, so this just adds their bottom margin.
                    let bottom_space = style.margin.bottom + style.padding.bottom;
                    (vec![], bottom_space, None)
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
                // The page is full, return it.
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