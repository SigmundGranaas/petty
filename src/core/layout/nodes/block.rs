use crate::core::idf::IRNode;
use crate::core::layout::elements::RectElement;
use crate::core::layout::node::{AnchorLocation, LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::page_break::PageBreakNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::border::Border;
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;

/// A `LayoutNode` for block-level containers like `<div>`.
/// It stacks its children vertically and is responsible for managing its own
/// margins, padding, and background color.
#[derive(Debug, Clone)]
pub struct BlockNode {
    id: Option<String>,
    children: Vec<Box<dyn LayoutNode>>,
    style: Arc<ComputedStyle>,
}

impl BlockNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (id, ir_children) = match node {
            IRNode::Block { meta, children } => (meta.id.clone(), children),
            IRNode::ListItem { meta, children } => (meta.id.clone(), children),
            _ => panic!("BlockNode must be created from a compatible IRNode"),
        };
        // In the refactored engine, children are built by the engine and passed in.
        // This constructor is now primarily for nodes that manage their own children, like ListItem.
        let children = engine.build_layout_node_children(ir_children, style.clone());
        Self { id, children, style }
    }
}

impl LayoutNode for BlockNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        let child_available_width = available_width
            - self.style.padding.left
            - self.style.padding.right
            - border_left_width
            - border_right_width;

        for child in &mut self.children {
            child.measure(engine, child_available_width);
        }
    }

    fn measure_content_height(&mut self, engine: &LayoutEngine, available_width: f32) -> f32 {
        if let Some(Dimension::Pt(h)) = self.style.height {
            return self.style.margin.top + h + self.style.margin.bottom;
        }
        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        let child_available_width = available_width
            - self.style.padding.left
            - self.style.padding.right
            - border_left_width
            - border_right_width;

        let content_height: f32 = self
            .children
            .iter_mut()
            .map(|c| c.measure_content_height(engine, child_available_width))
            .sum();

        self.style.margin.top
            + border_top_width
            + self.style.padding.top
            + content_height
            + self.style.padding.bottom
            + border_bottom_width
            + self.style.margin.bottom
    }

    fn measure_intrinsic_width(&self, engine: &LayoutEngine) -> f32 {
        let child_max_width = self
            .children
            .iter()
            .map(|c| c.measure_intrinsic_width(engine))
            .fold(0.0, f32::max);

        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        child_max_width
            + self.style.padding.left
            + self.style.padding.right
            + border_left_width
            + border_right_width
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.borrow_mut().insert(id.clone(), location);
        }

        // --- Vertical Margin Collapsing ---
        let margin_to_add = self.style.margin.top.max(ctx.last_v_margin);
        if !ctx.is_empty() && margin_to_add > ctx.available_height() {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }
        ctx.advance_cursor(margin_to_add);
        ctx.last_v_margin = 0.0;

        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        let block_start_y_in_ctx = ctx.cursor.1;
        ctx.advance_cursor(border_top_width + self.style.padding.top);
        let content_start_y_in_ctx = ctx.cursor.1;

        let child_bounds = geom::Rect {
            x: ctx.bounds.x + border_left_width + self.style.padding.left,
            y: ctx.bounds.y + content_start_y_in_ctx,
            width: ctx.bounds.width
                - self.style.padding.left
                - self.style.padding.right
                - border_left_width
                - border_right_width,
            height: ctx.available_height(),
        };
        let mut child_ctx = LayoutContext {
            engine: ctx.engine,
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: ctx.elements,
            last_v_margin: 0.0,
            local_page_index: ctx.local_page_index,
            defined_anchors: ctx.defined_anchors,
            index_entries: ctx.index_entries,
        };

        for (i, child) in self.children.iter_mut().enumerate() {
            match child.layout(&mut child_ctx) {
                Ok(LayoutResult::Full) => continue,
                Ok(LayoutResult::Partial(remainder)) => {
                    let content_height = child_ctx.cursor.1;
                    draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height);

                    ctx.cursor.1 = content_start_y_in_ctx + content_height + self.style.padding.bottom + border_bottom_width;
                    ctx.last_v_margin = child_ctx.last_v_margin;

                    let mut remaining_children = vec![remainder];
                    remaining_children.extend(self.children.drain((i + 1)..));

                    let next_page_block = Box::new(BlockNode {
                        id: self.id.clone(),
                        children: remaining_children,
                        style: self.style.clone(),
                    });
                    return Ok(LayoutResult::Partial(next_page_block));
                }
                Err(e) => {
                    log::warn!("Skipping child element that failed to lay out: {}", e);
                    continue;
                }
            }
        }

        let content_height = child_ctx.cursor.1;
        draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height);

        ctx.cursor.1 = content_start_y_in_ctx + content_height + self.style.padding.bottom + border_bottom_width;
        ctx.last_v_margin = self.style.margin.bottom.max(child_ctx.last_v_margin);

        Ok(LayoutResult::Full)
    }

    fn check_for_page_break(&mut self) -> Option<Option<String>> {
        if let Some(first_child) = self.children.first_mut() {
            if first_child.is::<PageBreakNode>() {
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

/// Helper to draw the background and borders for a block.
pub(super) fn draw_background_and_borders(
    ctx: &mut LayoutContext,
    style: &Arc<ComputedStyle>,
    start_y: f32,
    content_height: f32,
) {
    let border_top_width = style.border_top.as_ref().map_or(0.0, |b| b.width);
    let border_bottom_width = style.border_bottom.as_ref().map_or(0.0, |b| b.width);
    let border_left_width = style.border_left.as_ref().map_or(0.0, |b| b.width);
    let border_right_width = style.border_right.as_ref().map_or(0.0, |b| b.width);

    let inner_height = style.padding.top + content_height + style.padding.bottom;
    let total_height = border_top_width + inner_height + border_bottom_width;

    if total_height <= 0.0 {
        return;
    }

    // Draw background
    if style.background_color.is_some() {
        let bg_style = Arc::new(ComputedStyle {
            background_color: style.background_color.clone(),
            ..ComputedStyle::default()
        });
        let bg_rect = geom::Rect {
            x: border_left_width,
            y: border_top_width,
            width: ctx.bounds.width - border_left_width - border_right_width,
            height: inner_height,
        };
        let bg = PositionedElement {
            element: LayoutElement::Rectangle(RectElement),
            style: bg_style,
            ..PositionedElement::from_rect(bg_rect)
        };
        ctx.push_element_at(bg, 0.0, start_y);
    }

    let draw_border = |ctx: &mut LayoutContext, b: &Option<Border>, rect: geom::Rect| {
        if let Some(border) = b {
            if border.width > 0.0 {
                let border_style = Arc::new(ComputedStyle {
                    background_color: Some(border.color.clone()),
                    ..ComputedStyle::default()
                });
                let positioned_rect = PositionedElement {
                    element: LayoutElement::Rectangle(RectElement),
                    style: border_style,
                    ..PositionedElement::from_rect(rect)
                };
                ctx.push_element_at(positioned_rect, 0.0, start_y);
            }
        }
    };

    let bounds_width = ctx.bounds.width;
    draw_border(ctx, &style.border_top, geom::Rect { x: 0.0, y: 0.0, width: bounds_width, height: border_top_width });
    draw_border(ctx, &style.border_bottom, geom::Rect { x: 0.0, y: total_height - border_bottom_width, width: bounds_width, height: border_bottom_width });
    draw_border(ctx, &style.border_left, geom::Rect { x: 0.0, y: 0.0, width: border_left_width, height: total_height });
    draw_border(ctx, &style.border_right, geom::Rect { x: bounds_width - border_right_width, y: 0.0, width: border_right_width, height: total_height });
}

// Add a constructor to BlockNode for internal use.
impl BlockNode {
    pub fn new_from_children(
        id: Option<String>,
        children: Vec<Box<dyn LayoutNode>>,
        style: Arc<ComputedStyle>,
    ) -> Self {
        Self { id, children, style }
    }
}