// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/block.rs
use crate::core::idf::IRNode;
use crate::core::layout::elements::RectElement;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::page_break::PageBreakNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;

/// A `LayoutNode` for block-level containers like `<div>`.
/// It stacks its children vertically and is responsible for managing its own
/// margins, padding, and background color.
#[derive(Debug)]
pub struct BlockNode {
    children: Vec<Box<dyn LayoutNode>>,
    style: Arc<ComputedStyle>,
}

impl BlockNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let ir_children = match node {
            IRNode::Block { children, .. } | IRNode::ListItem { children, .. } => children,
            _ => panic!("BlockNode must be created from a compatible IRNode"),
        };
        let children = ir_children
            .iter()
            .map(|child_ir| engine.build_layout_node_tree(child_ir, style.clone()))
            .collect();
        Self { children, style }
    }

    pub fn new_root(
        node: &IRNode,
        engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
    ) -> Self {
        let style = engine.get_default_style(); // Root has no specific style
        let ir_children = match node {
            IRNode::Root(children) => children,
            _ => panic!("BlockNode (root) must be created from an IRNode::Root"),
        };
        let children = ir_children
            .iter()
            .map(|child_ir| engine.build_layout_node_tree(child_ir, style.clone()))
            .collect();
        Self { children, style }
    }
}

impl LayoutNode for BlockNode {
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
            return self.style.margin.top + h + self.style.margin.bottom;
        }

        let child_available_width = available_width - self.style.padding.left - self.style.padding.right;
        let content_height: f32 = self
            .children
            .iter_mut()
            .map(|c| c.measure_content_height(engine, child_available_width))
            .sum();

        self.style.margin.top
            + self.style.padding.top
            + content_height
            + self.style.padding.bottom
            + self.style.margin.bottom
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        if !ctx.is_empty() && self.style.margin.top > ctx.available_height() {
            return Ok(LayoutResult::Partial(Box::new(Self {
                children: std::mem::take(&mut self.children),
                style: self.style.clone(),
            })));
        }
        ctx.advance_cursor(self.style.margin.top);

        let content_start_y_in_ctx = ctx.cursor.1;

        let child_bounds = geom::Rect {
            x: ctx.bounds.x + self.style.padding.left,
            y: ctx.bounds.y + content_start_y_in_ctx + self.style.padding.top,
            width: ctx.bounds.width - self.style.padding.left - self.style.padding.right,
            height: ctx.available_height() - self.style.padding.top,
        };
        let mut child_ctx = LayoutContext {
            engine: ctx.engine,
            bounds: child_bounds,
            cursor: (0.0, 0.0), // Relative to its own bounds
            elements: unsafe { &mut *(ctx.elements as *mut Vec<PositionedElement>) },
        };

        for (i, child) in self.children.iter_mut().enumerate() {
            match child.layout(&mut child_ctx) {
                Ok(LayoutResult::Full) => continue,
                Ok(LayoutResult::Partial(remainder)) => {
                    let content_height = child_ctx.cursor.1;
                    draw_background(ctx, &self.style, content_start_y_in_ctx, content_height);

                    ctx.cursor.1 = content_start_y_in_ctx
                        + self.style.padding.top
                        + content_height
                        + self.style.padding.bottom;

                    let mut remaining_children = vec![remainder];
                    remaining_children.extend(self.children.drain((i + 1)..));

                    let next_page_block = Box::new(BlockNode {
                        children: remaining_children,
                        style: self.style.clone(),
                    });
                    return Ok(LayoutResult::Partial(next_page_block));
                }
                Err(e) => {
                    log::warn!("Skipping child element that failed to lay out: {}", e);
                    continue; // Skip this child and continue with the next one
                }
            }
        }

        let content_height = child_ctx.cursor.1;
        draw_background(ctx, &self.style, content_start_y_in_ctx, content_height);

        ctx.cursor.1 = content_start_y_in_ctx + self.style.padding.top + content_height + self.style.padding.bottom;
        ctx.advance_cursor(self.style.margin.bottom);

        Ok(LayoutResult::Full)
    }

    fn check_for_page_break(&mut self) -> Option<Option<String>> {
        if let Some(first_child) = self.children.first_mut() {
            if first_child.is::<PageBreakNode>() {
                // It is a PageBreakNode. We can downcast, remove it, and return its value.
                let page_break_node = self.children.remove(0).downcast::<PageBreakNode>().unwrap();
                return Some(page_break_node.master_name);
            }
        }
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Helper to draw the background rectangle for a block.
fn draw_background(
    ctx: &mut LayoutContext,
    style: &Arc<ComputedStyle>,
    start_y: f32,
    content_height: f32,
) {
    if style.background_color.is_some() {
        let bg_height = style.padding.top + content_height + style.padding.bottom;
        if bg_height > 0.0 {
            let bg = PositionedElement {
                x: 0.0,
                y: 0.0, // Will be offset by push_element_at
                width: ctx.bounds.width,
                height: bg_height,
                element: LayoutElement::Rectangle(RectElement),
                style: style.clone(),
            };
            ctx.push_element_at(bg, 0.0, start_y);
        }
    }
}

// Add a constructor to BlockNode for internal use.
impl BlockNode {
    pub fn new_from_children(children: Vec<Box<dyn LayoutNode>>, style: Arc<ComputedStyle>) -> Self {
        Self { children, style }
    }
}