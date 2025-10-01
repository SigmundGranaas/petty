// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/list_item.rs
use crate::core::idf::IRNode;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutElement, LayoutEngine, LayoutError, PositionedElement, TextElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::list::ListStyleType;
use std::any::Any;
use std::sync::Arc;

/// A `LayoutNode` for a single item within a list.
/// It is responsible for drawing its marker (bullet or number) and then
/// laying out its own children in an indented area.
#[derive(Debug)]
pub struct ListItemNode {
    children: Vec<Box<dyn LayoutNode>>,
    style: Arc<ComputedStyle>,
    marker_text: String,
}

impl ListItemNode {
    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        index: usize,
    ) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let ir_children = match node {
            IRNode::ListItem { children, .. } => children,
            _ => panic!("ListItemNode must be created from an IRNode::ListItem"),
        };
        let children = ir_children
            .iter()
            .map(|child_ir| engine.build_layout_node_tree(child_ir, style.clone()))
            .collect();

        // Determine marker content based on style.
        let marker_text = match style.list_style_type {
            ListStyleType::Disc => "•".to_string(),
            ListStyleType::Circle => "◦".to_string(),
            ListStyleType::Square => "▪".to_string(),
            ListStyleType::Decimal => format!("{}.", index),
            ListStyleType::None => String::new(),
        };

        Self {
            children,
            style,
            marker_text,
        }
    }
}

impl LayoutNode for ListItemNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        // Correctly calculate indented width for children measurement
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let indent = if !self.marker_text.is_empty() {
            let marker_width = engine.measure_text_width(&self.marker_text, &self.style);
            let marker_spacing = self.style.font_size * MARKER_SPACING_FACTOR;
            marker_width + marker_spacing
        } else {
            0.0
        };
        let child_available_width = available_width
            - self.style.padding.left
            - self.style.padding.right
            - indent;

        for child in &mut self.children {
            child.measure(engine, child_available_width);
        }
    }

    fn measure_content_height(&mut self, engine: &LayoutEngine, available_width: f32) -> f32 {
        // Correctly calculate indented width for children measurement
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let indent = if !self.marker_text.is_empty() {
            let marker_width = engine.measure_text_width(&self.marker_text, &self.style);
            let marker_spacing = self.style.font_size * MARKER_SPACING_FACTOR;
            marker_width + marker_spacing
        } else {
            0.0
        };
        let child_available_width = available_width
            - self.style.padding.left
            - self.style.padding.right
            - indent;

        // Determine the height of the child content.
        let content_height = if let Some(Dimension::Pt(h)) = self.style.height {
            h
        } else {
            self.children
                .iter_mut()
                .map(|c| c.measure_content_height(engine, child_available_width))
                .sum()
        };

        // The total inner height of the content box is the greater of the children's
        // stacked height (plus padding) and the minimum height required for the marker line.
        let inner_height = self.style.padding.top
            + content_height.max(self.style.line_height)
            + self.style.padding.bottom;

        // The total space taken up by the element is its inner box height plus vertical margins.
        self.style.margin.top + inner_height + self.style.margin.bottom
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        // 1. Handle top margin
        if !ctx.is_empty() && self.style.margin.top > ctx.available_height() {
            return Ok(LayoutResult::Partial(Box::new(Self {
                children: std::mem::take(&mut self.children),
                style: self.style.clone(),
                marker_text: self.marker_text.clone(),
            })));
        }
        ctx.advance_cursor(self.style.margin.top);

        // Record the Y position where the content box (padding + content) will start.
        let content_start_y_in_ctx = ctx.cursor.1;

        // 2. Handle Marker Layout
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let indent;
        if !self.marker_text.is_empty() {
            // Check if just the first line would cause a page break.
            let required_height_for_first_line =
                self.style.padding.top + self.style.line_height + self.style.padding.bottom;
            if required_height_for_first_line > ctx.available_height() && !ctx.is_empty() {
                ctx.cursor.1 -= self.style.margin.top; // Roll back margin advance
                return Ok(LayoutResult::Partial(Box::new(Self {
                    children: std::mem::take(&mut self.children),
                    style: self.style.clone(),
                    marker_text: self.marker_text.clone(), // Keep marker for next page
                })));
            }

            let marker_width = ctx.engine.measure_text_width(&self.marker_text, &self.style);
            let marker_spacing = self.style.font_size * MARKER_SPACING_FACTOR;
            indent = marker_width + marker_spacing;

            let marker_box = PositionedElement {
                x: 0.0,
                y: 0.0,
                width: marker_width,
                height: self.style.line_height,
                element: LayoutElement::Text(TextElement {
                    content: self.marker_text.clone(),
                    href: None,
                }),
                style: self.style.clone(),
            };
            ctx.push_element_at(
                marker_box,
                self.style.padding.left,
                content_start_y_in_ctx + self.style.padding.top,
            );
        } else {
            indent = 0.0;
        }

        // 3. Setup child context
        let child_bounds = geom::Rect {
            x: ctx.bounds.x + self.style.padding.left + indent,
            y: ctx.bounds.y + content_start_y_in_ctx + self.style.padding.top,
            width: ctx.bounds.width - self.style.padding.left - self.style.padding.right - indent,
            height: ctx.available_height() - self.style.padding.top,
        };
        let mut child_ctx = LayoutContext {
            engine: ctx.engine,
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: unsafe { &mut *(ctx.elements as *mut Vec<PositionedElement>) },
        };

        // 4. Layout children
        for (i, child) in self.children.iter_mut().enumerate() {
            match child.layout(&mut child_ctx)? {
                LayoutResult::Full => continue,
                LayoutResult::Partial(remainder) => {
                    let height_used_by_children = child_ctx.cursor.1;
                    let total_content_height = self.style.padding.top
                        + height_used_by_children.max(self.style.line_height)
                        + self.style.padding.bottom;

                    ctx.cursor.1 = content_start_y_in_ctx + total_content_height;

                    let mut remaining_children = vec![remainder];
                    remaining_children.extend(self.children.drain((i + 1)..));

                    let next_page_item = Box::new(ListItemNode {
                        children: remaining_children,
                        style: self.style.clone(),
                        marker_text: String::new(), // No marker on subsequent pages
                    });
                    return Ok(LayoutResult::Partial(next_page_item));
                }
            }
        }

        // 5. Finalize height and advance cursor for full layout
        let height_used_by_children = child_ctx.cursor.1;
        let total_content_height = self.style.padding.top
            + height_used_by_children.max(self.style.line_height)
            + self.style.padding.bottom;

        ctx.cursor.1 = content_start_y_in_ctx + total_content_height;

        // 6. Handle bottom margin
        ctx.advance_cursor(self.style.margin.bottom);

        Ok(LayoutResult::Full)
    }
}