// src/layout/page.rs

//! The stateful page iterator for the positioning pass.

use super::block;
use super::elements::PositionedElement;
use super::flex;
use super::image;
use super::style;
use super::table;
use super::text;
use super::{IRNode, LayoutEngine, WorkItem};
use crate::stylesheet::Margins;

/// A stateful iterator that performs the **Positioning Pass** of the layout algorithm.
/// It consumes a pre-measured `IRNode` tree and yields pages of positioned elements.
pub struct PageIterator<'a> {
    engine: &'a LayoutEngine,
    work_stack: Vec<(WorkItem, style::ComputedStyle)>,
    current_y: f32,
    page_width: f32,
    page_height: f32,
    margins: &'a Margins,
    is_finished: bool,
}

impl<'a> PageIterator<'a> {
    /// Creates a new `PageIterator`. This is typically called by the `LayoutEngine`.
    pub fn new(annotated_tree: IRNode, engine: &'a LayoutEngine) -> Self {
        let (page_width, page_height) = style::get_page_dimensions(&engine.stylesheet);
        let margins = &engine.stylesheet.page.margins;
        let mut work_stack = Vec::new();
        let default_style = engine.get_default_style();

        if let IRNode::Root(children) = annotated_tree {
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
}

impl<'a> Iterator for PageIterator<'a> {
    type Item = Vec<PositionedElement>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_finished {
            return None;
        }

        let mut page_elements = Vec::new();
        self.current_y = self.margins.top;

        while let Some((work_item, parent_style)) = self.work_stack.pop() {
            let remaining_height = self.page_height - self.margins.bottom - self.current_y;
            let current_x = self.content_left();
            let available_width = self.content_width();

            let (mut elements, consumed_height, pending_work) = match work_item {
                WorkItem::Node(mut node) => {
                    // Compute style once before layout.
                    let style = self.engine.compute_style(node.style_name(), &parent_style);

                    // Check which node types manage their own stack.
                    // This is key to avoiding double-pushing an EndNode.
                    let pushes_own_end_node = matches!(
                        node,
                        IRNode::Block { .. } | IRNode::List { .. } | IRNode::ListItem { .. }
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
        if !page_elements.is_empty() {
            Some(page_elements)
        } else {
            None
        }
    }
}

// --- Layout Dispatch ---

impl<'a> PageIterator<'a> {
    /// Dispatches to the correct layout function based on the node type.
    /// Returns the elements, consumed height, and any work that needs to be deferred to the next page.
    fn layout_node(
        &mut self,
        node: &mut IRNode,
        style: &style::ComputedStyle, // CHANGED: Take computed style directly
        available_width: f32,
        available_height: f32,
    ) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
        // REMOVED: `let style = ...` line is no longer needed here.
        let margin_top = style.margin.top;

        if margin_top >= available_height {
            log::trace!("Node {:?} top margin ({:.2}) >= available_height ({:.2}), pushing to next page.", node.style_name(), margin_top, available_height);
            return (vec![], 0.0, Some(WorkItem::Node(node.clone())));
        }

        let max_height_for_node_content_and_padding_and_bottom_margin = available_height - margin_top;

        let (mut elements, content_only_height_produced, pending_content) = match node {
            IRNode::Paragraph { children, style_name } => {
                let content_width = available_width - style.padding.left - style.padding.right;
                let max_height_for_paragraph_text =
                    max_height_for_node_content_and_padding_and_bottom_margin
                        - style.padding.top - style.padding.bottom
                        - style.margin.bottom;

                let (mut els, text_lines_height_consumed, remaining_inlines) =
                    text::layout_paragraph(self.engine, children, &style, content_width, max_height_for_paragraph_text.max(0.0));

                for el in &mut els { el.x += style.padding.left; el.y += style.padding.top; }

                let consumed_height_on_page_for_para_portion = text_lines_height_consumed + style.padding.top + style.padding.bottom;

                if let Some(rem_inlines) = remaining_inlines {
                    let pending_node = IRNode::Paragraph { style_name: style_name.clone(), children: rem_inlines };
                    log::trace!("Paragraph {:?} split. Text height consumed: {:.2}. Pending work.", style_name, text_lines_height_consumed);
                    (els, consumed_height_on_page_for_para_portion, Some(WorkItem::Node(pending_node)))
                } else {
                    log::trace!("Paragraph {:?} fit completely. Total height for portion: {:.2}", style_name, consumed_height_on_page_for_para_portion);
                    (els, consumed_height_on_page_for_para_portion, None)
                }
            }
            IRNode::Block { children, .. } | IRNode::List { children, .. } => {
                let (els, height, pending) = block::layout_block(&mut self.work_stack, children, &style);
                (els, height + style.padding.top + style.padding.bottom, pending)
            }
            IRNode::ListItem { children, .. } => {
                let (els, height, pending) = block::layout_list_item(&mut self.work_stack, children, &style);
                (els, height + style.padding.top + style.padding.bottom, pending)
            }
            IRNode::FlexContainer { children, .. } => {
                let (els, height, pending) = flex::layout_flex_container(self.engine, children, &style, available_width);
                let total_height = height + style.padding.top + style.padding.bottom;
                if margin_top + total_height + style.margin.bottom > available_height {
                    (vec![], 0.0, Some(WorkItem::Node(node.clone())))
                } else {
                    (els, total_height, pending)
                }
            }
            IRNode::Image { src, data, .. } => {
                let (els, height, pending) = image::layout_image(src, data.as_ref(), &style, available_width);
                let total_height = height + style.padding.top + style.padding.bottom;
                if margin_top + total_height + style.margin.bottom > available_height {
                    (vec![], 0.0, Some(WorkItem::Node(node.clone())))
                } else {
                    (els, total_height, pending)
                }
            }
            IRNode::Table { header, body, calculated_widths, .. } => {
                let (els, height, pending) = table::layout_table(self.engine, header.as_deref_mut(), body, &style, calculated_widths);
                let total_height = height + style.padding.top + style.padding.bottom;
                if margin_top + total_height + style.margin.bottom > available_height {
                    (vec![], 0.0, Some(WorkItem::Node(node.clone())))
                } else {
                    (els, total_height, pending)
                }
            }
            _ => (vec![], 0.0, None),
        };

        if let Some(pending) = pending_content {
            let height_consumed_on_page_for_current_node = margin_top + content_only_height_produced;
            return (elements, height_consumed_on_page_for_current_node, Some(pending));
        }

        let total_height_for_this_node_including_all_margins_paddings = margin_top + content_only_height_produced + style.margin.bottom;

        if total_height_for_this_node_including_all_margins_paddings > available_height && !elements.is_empty() {
            (vec![], 0.0, Some(WorkItem::Node(node.clone())))
        } else {
            if style.background_color.is_some() {
                block::add_background(&mut elements, &style, available_width, content_only_height_produced);
            }
            let height_consumed_on_page_for_current_node = margin_top + content_only_height_produced;
            // FIXED: Return None for pending_work. The iterator loop now handles the EndNode.
            (elements, height_consumed_on_page_for_current_node, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::{InlineNode, LayoutUnit};
    use crate::stylesheet::{PageLayout, PageSize, Stylesheet};
    use serde_json::Value;

    fn create_test_engine_with_small_page() -> LayoutEngine {
        let stylesheet = Stylesheet {
            page: PageLayout {
                size: PageSize::Custom { width: 500.0, height: 100.0 },
                margins: Margins { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 },
                ..Default::default()
            },
            ..Default::default()
        };
        LayoutEngine::new(stylesheet)
    }

    #[test]
    fn test_paragraph_splits_across_pages() {
        let engine = create_test_engine_with_small_page();
        // line_height is 14.4. Page content height is 100 - 10 - 10 = 80.
        // 80 / 14.4 = 5.55. So, 5 lines should fit on page 1.
        // We will create text that generates more than 5 lines.
        let long_text = "This is a very long line of text designed to wrap multiple times and exceed the height of a single small page. ".repeat(5);

        let tree = IRNode::Root(vec![IRNode::Paragraph {
            style_name: None,
            children: vec![InlineNode::Text(long_text)],
        }]);

        let layout_unit = LayoutUnit { tree, context: Value::Null };

        let mut page_iter = engine.paginate_tree(&layout_unit).unwrap();

        // Page 1
        let page1 = page_iter.next().expect("Should generate page 1");
        assert!(!page1.is_empty(), "Page 1 should have content");
        let last_element_p1 = page1.last().unwrap();
        // Check that the last element on page 1 is near the bottom margin
        assert!(last_element_p1.y + last_element_p1.height < 100.0 - 10.0, "Content on page 1 should not exceed bottom margin");
        assert!(page1.len() >= 5, "Page 1 should have at least 5 lines");

        // Page 2
        let page2 = page_iter.next().expect("Should generate page 2");
        assert!(!page2.is_empty(), "Page 2 should have content");
        let first_element_p2 = page2.first().unwrap();
        // Check that content on page 2 starts back at the top margin
        assert!((first_element_p2.y - engine.stylesheet.page.margins.top).abs() < 1.0, "Content on page 2 should start near the top margin");

        // End of document
        assert!(page_iter.next().is_none(), "Should be no more than 2 pages");
    }
}