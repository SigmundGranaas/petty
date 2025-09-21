// src/layout/page.rs
//! The stateful page iterator for the positioning pass.

use super::block;
use super::elements::PositionedElement;
use super::flex;
use super::image;
use super::style;
use super::table;
use super::text;
use super::{IRNode, LayoutEngine, LayoutUnit, WorkItem};
use crate::stylesheet::Margins;
use std::sync::Arc;

/// A stateful iterator that performs the **Positioning Pass** of the layout algorithm.
/// It consumes a pre-measured `IRNode` tree and yields pages of positioned elements.
pub struct PageIterator<'a> {
    engine: &'a LayoutEngine,
    work_stack: Vec<(WorkItem, Arc<style::ComputedStyle>)>,
    current_y: f32,
    page_width: f32,
    page_height: f32,
    content_bottom: f32, // The Y coordinate where the content area ends
    margins: &'a Margins,
    is_finished: bool,
}

impl<'a> PageIterator<'a> {
    /// Creates a new `PageIterator`. This is typically called by the `LayoutEngine`.
    pub fn new(layout_unit: LayoutUnit, engine: &'a LayoutEngine) -> Self {
        let (page_width, page_height) = style::get_page_dimensions(&engine.stylesheet);
        let margins = &engine.stylesheet.page.margins;
        // FIX: Calculate the end of the content area by reserving space for the footer.
        let content_bottom = margins.bottom + engine.stylesheet.page.footer_height;

        let mut work_stack = Vec::new();
        let default_style = engine.get_default_style();

        if let IRNode::Root(children) = layout_unit.tree {
            // Push children onto the stack in reverse order to process them correctly.
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
        }
    }

    /// The available width for content within the page margins.
    fn content_width(&self) -> f32 {
        self.page_width - self.margins.left - self.margins.right
    }

    /// The starting horizontal position for content.
    fn content_left(&self) -> f32 {
        self.margins.left
    }

    /// Dispatches to the correct layout function based on the node type.
    /// Returns the elements, consumed height, and any work that needs to be deferred to the next page.
    fn layout_node(
        &mut self,
        node: &mut IRNode,
        style: &Arc<style::ComputedStyle>,
        available_width: f32,
        available_height: f32,
    ) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
        let margin_top = style.margin.top;

        if margin_top >= available_height {
            log::trace!("Node {:?} top margin ({:.2}) >= available_height ({:.2}), pushing to next page.", node.style_name(), margin_top, available_height);
            return (vec![], 0.0, Some(WorkItem::Node(node.clone())));
        }

        let inner_available_height = available_height - margin_top;

        let (mut elements, mut content_height, pending_content) = match node {
            IRNode::Paragraph {
                children,
                style_name,
                style_override,
            } => text::layout_paragraph_node(
                self.engine,
                children,
                style_name,
                style_override,
                style,
                available_width,
                inner_available_height,
            ),
            IRNode::Block { children, .. } | IRNode::List { children, .. } => {
                block::layout_block(&mut self.work_stack, children, style)
            }
            IRNode::ListItem { children, .. } => {
                block::layout_list_item(&mut self.work_stack, children, style)
            }
            IRNode::FlexContainer { children, .. } => {
                flex::layout_flex_container(self.engine, children, style, available_width)
            }
            IRNode::Image { src, data, .. } => {
                image::layout_image(src, data.as_ref(), style, available_width)
            }
            // FIX: Corrected match ergonomics for Rust 2021 edition
            &mut IRNode::Table {
                ref style_name,
                ref style_override,
                ref columns,
                ref mut header,
                ref mut body,
                ref calculated_widths,
                ..
            } => table::layout_table_node(
                self.engine,
                style_name,
                style_override,
                columns,
                header,
                body,
                calculated_widths,
                style,
                inner_available_height,
            ),
            _ => (vec![], 0.0, None),
        };

        // A node's layout function might return 0 (e.g., an image with no data),
        // but if the style specifies a height, we must honor it.
        if let Some(h) = style.height {
            content_height = content_height.max(h);
        }

        for el in &mut elements {
            el.x += style.padding.left;
            el.y += style.padding.top;
        }

        let height_with_padding = content_height + style.padding.top + style.padding.bottom;

        // --- INFINITE LOOP PREVENTION ---
        // If an item has no pending content (i.e., it's an unbreakable block like a table),
        // we check if it fits in the remaining space.
        if pending_content.is_none() {
            let total_node_height = margin_top + height_with_padding + style.margin.bottom;
            if total_node_height > available_height {
                // This node doesn't fit in the remaining space. Now we must check if it's
                // fundamentally too big to fit on *any* page.
                let fresh_page_content_height = self.page_height - self.margins.top - self.content_bottom;
                if total_node_height > fresh_page_content_height {
                    // This node is too big even for a fresh page.
                    log::error!(
                        "Node with style '{:?}' has a height of {:.2} which exceeds the total page content height of {:.2}. The node will be skipped to prevent an infinite loop.",
                        node.style_name(), total_node_height, fresh_page_content_height
                    );
                    // Discard the node by not returning any pending work.
                    return (vec![], 0.0, None);
                } else {
                    // It's not oversized, it just needs a new page. Push it back onto the stack.
                    return (vec![], 0.0, Some(WorkItem::Node(node.clone())));
                }
            }
        }

        if style.background_color.is_some() || style.border_bottom.is_some() {
            let total_content_width = available_width - style.margin.left - style.margin.right;
            block::add_background(
                &mut elements,
                style,
                total_content_width,
                height_with_padding,
            );
        }

        // Shift the entire block of elements (content + background) down by the top margin.
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
            work_processed = true; // We popped an item, so we are doing work.

            // FIX: Use the new `content_bottom` field to calculate remaining height.
            let remaining_height = self.page_height - self.content_bottom - self.current_y;
            let current_x = self.content_left();
            let available_width = self.content_width();

            let (mut elements, consumed_height, pending_work) = match work_item {
                WorkItem::Node(mut node) => {
                    // Compute style once before layout.
                    let style = self.engine.compute_style(
                        node.style_name(),
                        node.style_override(),
                        &parent_style,
                    );

                    // FIX: All container types that manage children via the stack should be included here.
                    let pushes_own_end_node = matches!(
                        node,
                        IRNode::Block { .. }
                            | IRNode::List { .. }
                            | IRNode::ListItem { .. }
                            // REMOVED from original file: | IRNode::FlexContainer { .. }
                    );

                    // Pass the computed style directly to layout_node.
                    let (els, consumed, pending) = self.layout_node(
                        &mut node,
                        &style,
                        available_width,
                        remaining_height,
                    );

                    // If layout succeeded and the node doesn't manage its own stack,
                    // we are now responsible for pushing its EndNode to handle bottom margin.
                    if pending.is_none() && !pushes_own_end_node {
                        self.work_stack
                            .push((WorkItem::EndNode(style), parent_style.clone()));
                    }
                    (els, consumed, pending)
                }
                WorkItem::EndNode(style) => (vec![], style.margin.bottom, None),
            };

            if let Some(pending) = pending_work {
                // A page break occurred. Push the pending work back for the next page.
                self.work_stack.push((pending, parent_style));
                // Add any elements that *did* fit on this page before the break.
                for el in &mut elements {
                    el.x += current_x;
                    el.y += self.current_y;
                }
                page_elements.extend(elements);
                // Return the completed page.
                return Some(page_elements);
            }

            // No page break, so add the generated elements to the current page.
            for el in &mut elements {
                el.x += current_x;
                el.y += self.current_y;
            }
            page_elements.extend(elements);
            self.current_y += consumed_height;
        }

        // All work is done.
        self.is_finished = true;

        // --- FIX: Return a page if work was done, even if it produced no elements. ---
        // This correctly handles pages with invisible placeholders.
        if work_processed {
            Some(page_elements)
        } else {
            None
        }
    }
}