// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/list_item.rs
use crate::core::idf::IRNode;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::list::ListStyleType;
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

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        for child in &mut self.children {
            child.measure(engine, available_width);
        }
    }
    fn measure_content_height(&mut self, engine: &LayoutEngine, available_width: f32) -> f32 {
        if let Some(Dimension::Pt(h)) = self.style.height {
            return h; // List item height doesn't include its margins
        }
        self.children
            .iter_mut()
            .map(|c| c.measure_content_height(engine, available_width))
            .sum()
    }
    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        const MARKER_SPACING_FACTOR: f32 = 0.4;

        // --- 1. Handle Marker Layout ---
        let indent;
        if !self.marker_text.is_empty() {
            let marker_width = ctx.engine.measure_text_width(&self.marker_text, &self.style);
            let marker_spacing = self.style.font_size * MARKER_SPACING_FACTOR;
            indent = marker_width + marker_spacing;

            let marker_box = PositionedElement {
                x: 0.0,
                y: 0.0,
                width: marker_width,
                height: self.style.line_height,
                element: crate::core::layout::LayoutElement::Text(
                    crate::core::layout::TextElement {
                        content: self.marker_text.clone(),
                        href: None,
                    },
                ),
                style: self.style.clone(),
            };
            if self.style.line_height > ctx.available_height() && !ctx.is_empty() {
                return Ok(LayoutResult::Partial(Box::new(Self {
                    children: std::mem::take(&mut self.children),
                    style: self.style.clone(),
                    marker_text: self.marker_text.clone(),
                })));
            }
            ctx.push_element(marker_box);
        } else {
            indent = 0.0;
        }

        let content_start_y_in_ctx = ctx.cursor.1;
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

        for (i, child) in self.children.iter_mut().enumerate() {
            match child.layout(&mut child_ctx)? {
                LayoutResult::Full => continue,
                LayoutResult::Partial(remainder) => {
                    let height_used_by_children = child_ctx.cursor.1;
                    let total_height = (self.style.padding.top
                        + height_used_by_children
                        + self.style.padding.bottom)
                        .max(self.style.line_height);
                    ctx.advance_cursor(total_height);

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

        let height_used_by_children = child_ctx.cursor.1;
        let total_height = (self.style.padding.top
            + height_used_by_children
            + self.style.padding.bottom)
            .max(self.style.line_height);
        ctx.advance_cursor(total_height);

        Ok(LayoutResult::Full)
    }
}