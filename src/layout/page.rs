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
    content_bottom: f32, // The Y coordinate where the content area ends
    margins: &'a Margins,
    is_finished: bool,
}

impl<'a> PageIterator<'a> {
    /// Creates a new `PageIterator`. This is typically called by the `LayoutEngine`.
    pub fn new(annotated_tree: IRNode, engine: &'a LayoutEngine) -> Self {
        let (page_width, page_height) = style::get_page_dimensions(&engine.stylesheet);
        let margins = &engine.stylesheet.page.margins;
        // FIX: Calculate the end of the content area by reserving space for the footer.
        let content_bottom = margins.bottom + engine.stylesheet.page.footer_height;

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
        style: &style::ComputedStyle,
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
            IRNode::Table {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::{
        IRNode, InlineNode, LayoutUnit, TableBody, TableCell, TableColumnDefinition, TableHeader,
        TableRow,
    };
    use crate::layout::LayoutElement;
    use crate::stylesheet::{ElementStyle, Margins, PageLayout, PageSize, Stylesheet};
    use serde_json::Value;
    use std::collections::HashMap;

    fn create_test_engine(page_height: f32) -> LayoutEngine {
        let stylesheet = Stylesheet {
            page: PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: page_height,
                },
                margins: Margins {
                    top: 10.0,
                    right: 10.0,
                    bottom: 10.0,
                    left: 10.0,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        LayoutEngine::new(stylesheet)
    }

    fn create_test_engine_with_styles(
        page_height: f32,
        styles: HashMap<String, ElementStyle>,
    ) -> LayoutEngine {
        let stylesheet = Stylesheet {
            page: PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: page_height,
                },
                margins: Margins {
                    top: 10.0,
                    right: 10.0,
                    bottom: 10.0,
                    left: 10.0,
                },
                ..Default::default()
            },
            styles,
            ..Default::default()
        };
        LayoutEngine::new(stylesheet)
    }

    #[test]
    fn test_single_page_layout() {
        let engine = create_test_engine(500.0);
        let tree = IRNode::Root(vec![
            IRNode::Paragraph {
                style_name: None,
                style_override: None,
                children: vec![InlineNode::Text("Hello".to_string())],
            },
            IRNode::Paragraph {
                style_name: None,
                style_override: None,
                children: vec![InlineNode::Text("World".to_string())],
            },
        ]);

        let layout_unit = LayoutUnit {
            tree,
            context: Value::Null,
        };
        let mut page_iter = engine.paginate_tree(&layout_unit).unwrap();

        let page1 = page_iter.next().expect("Should have one page");
        assert_eq!(page1.len(), 2);
        assert!((page1[0].y - 10.0).abs() < 0.1); // Starts at top margin
        assert!((page1[1].y - (10.0 + 14.4)).abs() < 0.1); // Second element is one line height down

        assert!(page_iter.next().is_none(), "Should only be one page");
    }

    #[test]
    fn test_paragraph_splits_across_pages() {
        let engine = create_test_engine(100.0);
        // Page content height is 100 - 10 - 10 = 80.
        // Default line height is 14.4. 80 / 14.4 = 5.55 -> 5 lines fit.
        let long_text = "This is a very long line of text designed to wrap multiple times and exceed the height of a single small page. ".repeat(5);

        let tree = IRNode::Root(vec![IRNode::Paragraph {
            style_name: None,
            style_override: None,
            children: vec![InlineNode::Text(long_text)],
        }]);

        let layout_unit = LayoutUnit {
            tree,
            context: Value::Null,
        };
        let mut page_iter = engine.paginate_tree(&layout_unit).unwrap();

        // Page 1
        let page1 = page_iter.next().expect("Should generate page 1");
        assert!(!page1.is_empty());
        let last_element_p1 = page1.last().unwrap();
        assert!(
            last_element_p1.y + last_element_p1.height < 90.0,
            "Content on page 1 should not exceed bottom margin"
        );
        assert!(
            page1.len() >= 5,
            "Page 1 should have at least 5 lines, but had {}",
            page1.len()
        );

        // Page 2
        let page2 = page_iter.next().expect("Should generate page 2");
        assert!(!page2.is_empty());
        let first_element_p2 = page2.first().unwrap();
        assert!(
            (first_element_p2.y - engine.stylesheet.page.margins.top).abs() < 1.0,
            "Content on page 2 should start near the top margin"
        );

        assert!(page_iter.next().is_none(), "Should be exactly two pages");
    }

    #[test]
    fn test_margins_are_applied() {
        let mut styles = HashMap::new();
        styles.insert(
            "margined".to_string(),
            ElementStyle {
                margin: Some(Margins {
                    top: 20.0,
                    bottom: 5.0,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let engine = create_test_engine_with_styles(500.0, styles);

        let tree = IRNode::Root(vec![
            IRNode::Paragraph {
                style_name: None,
                style_override: None,
                children: vec![InlineNode::Text("First".to_string())],
            },
            IRNode::Paragraph {
                style_name: Some("margined".to_string()),
                style_override: None,
                children: vec![InlineNode::Text("Second".to_string())],
            },
            IRNode::Paragraph {
                style_name: None,
                style_override: None,
                children: vec![InlineNode::Text("Third".to_string())],
            },
        ]);
        let layout_unit = LayoutUnit {
            tree,
            context: Value::Null,
        };

        let mut page_iter = engine.paginate_tree(&layout_unit).unwrap();
        let page1 = page_iter.next().unwrap();

        assert_eq!(page1.len(), 3);

        let default_style = style::get_default_style();
        let margined_style =
            engine.compute_style(Some("margined"), None, &default_style);

        // Position of first element's content box
        let y0 = 10.0 + default_style.margin.top;
        assert!((page1[0].y - y0).abs() < 0.1);
        let height0 = page1[0].height + default_style.padding.top + default_style.padding.bottom;

        // Position of second element's content box
        let y1 = y0 + height0 + default_style.margin.bottom + margined_style.margin.top;
        assert!((page1[1].y - y1).abs() < 0.1);
        let height1 = page1[1].height + margined_style.padding.top + margined_style.padding.bottom;

        // Position of third element's content box
        let y2 = y1 + height1 + margined_style.margin.bottom + default_style.margin.top;
        assert!((page1[2].y - y2).abs() < 0.1);
    }

    #[test]
    fn test_node_pushed_to_next_page_if_it_does_not_fit() {
        let mut styles = HashMap::new();
        styles.insert(
            "tall_box".to_string(),
            ElementStyle {
                height: Some(crate::stylesheet::Dimension::Pt(50.0)),
                // Add a background color so the block generates a renderable rectangle.
                background_color: Some(crate::stylesheet::Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 1.0,
                }),
                ..Default::default()
            },
        );
        let engine = create_test_engine_with_styles(100.0, styles); // Content height = 80

        let tree = IRNode::Root(vec![
            // This first paragraph will take up space
            IRNode::Paragraph {
                style_name: None,
                style_override: None,
                children: vec![InlineNode::Text("Line\nLine\nLine\nLine".to_string())], // 4 lines * 14.4 = 57.6pts
            },
            // The remaining space is 80 - 57.6 = 22.4. The block is 50pts tall, so it shouldn't fit.
            // Use a Block instead of an Image to make the test more robust.
            IRNode::Block {
                style_name: Some("tall_box".to_string()),
                style_override: None,
                children: vec![],
            },
        ]);

        let layout_unit = LayoutUnit {
            tree,
            context: Value::Null,
        };
        let mut page_iter = engine.paginate_tree(&layout_unit).unwrap();

        let page1 = page_iter.next().expect("Page 1 should exist");
        assert_eq!(
            page1.len(),
            4,
            "Page 1 should only contain the text lines"
        );

        let page2 = page_iter.next().expect("Page 2 should exist");
        assert_eq!(
            page2.len(),
            1,
            "Page 2 should contain the block's background rectangle"
        );

        // Verify we got the correct element (a Rectangle from the background).
        assert!(matches!(
            page2[0].element,
            crate::layout::LayoutElement::Rectangle(_)
        ));

        // Verify its position and size.
        assert!(
            (page2[0].y - 10.0).abs() < 0.1,
            "Block should start at the top margin of the new page"
        );
        assert!(
            (page2[0].height - 50.0).abs() < 0.1,
            "Block should have the correct height from its style"
        );
    }

    #[test]
    fn test_table_splits_across_pages_with_header_repeat() {
        // Page height 150, content height 130.
        // Line height ~14.4. Header is 1 row, so ~14.4 height.
        // Remaining content height for body: 130 - 14.4 = 115.6.
        // 115.6 / 14.4 = ~8 rows can fit. We will create 10 body rows.
        let engine = create_test_engine(150.0);
        let body_rows: Vec<TableRow> = (0..10)
            .map(|i| TableRow {
                cells: vec![TableCell {
                    style_name: None,
                    style_override: None,
                    children: vec![IRNode::Paragraph {
                        style_name: None,
                        style_override: None,
                        children: vec![InlineNode::Text(format!("Row {}", i + 1))],
                    }],
                }],
            })
            .collect();
        let table = IRNode::Table {
            style_name: None,
            style_override: None,
            columns: vec![TableColumnDefinition {
                width: None,
                style: None,
                header_style: None,
            }],
            calculated_widths: vec![480.0], // pre-calculated for simplicity
            header: Some(Box::new(TableHeader {
                rows: vec![TableRow {
                    cells: vec![TableCell {
                        style_name: None,
                        style_override: None,
                        children: vec![IRNode::Paragraph {
                            style_name: None,
                            style_override: None,
                            children: vec![InlineNode::Text("Header".to_string())],
                        }],
                    }],
                }],
            })),
            body: Box::new(TableBody { rows: body_rows }),
        };

        let layout_unit = LayoutUnit {
            tree: IRNode::Root(vec![table]),
            context: Value::Null,
        };
        let mut page_iter = engine.paginate_tree(&layout_unit).unwrap();

        // Page 1
        let page1 = page_iter.next().expect("Page 1 should be generated");
        assert!(!page1.is_empty(), "Page 1 should not be empty");
        let page1_text_elements: Vec<&String> = page1
            .iter()
            .filter_map(|el| match &el.element {
                LayoutElement::Text(t) => Some(&t.content),
                _ => None,
            })
            .collect();

        assert_eq!(page1_text_elements[0], "Header", "Page 1 should have header");
        assert!(
            page1_text_elements.len() > 2 && page1_text_elements.len() < 11,
            "Page 1 should have the header and some, but not all, rows. Had {} text elements.",
            page1_text_elements.len()
        );
        assert_eq!(
            page1_text_elements.last().unwrap().as_str(),
            "Row 8",
            "Page 1 should end with Row 8"
        );

        // Page 2
        let page2 = page_iter.next().expect("Page 2 should be generated");
        assert!(!page2.is_empty(), "Page 2 should not be empty");
        let page2_text_elements: Vec<&String> = page2
            .iter()
            .filter_map(|el| match &el.element {
                LayoutElement::Text(t) => Some(&t.content),
                _ => None,
            })
            .collect();

        assert_eq!(page2_text_elements[0], "Header", "Page 2 should repeat header");
        assert!(
            page2_text_elements.len() >= 3,
            "Page 2 should have the header and the remaining rows"
        );
        assert_eq!(
            page2_text_elements[1], "Row 9",
            "Page 2 should start with Row 9"
        );
        assert_eq!(
            page2_text_elements.last().unwrap().as_str(),
            "Row 10",
            "Page 2 should end with Row 10"
        );

        assert!(page_iter.next().is_none(), "There should only be two pages");
    }
}