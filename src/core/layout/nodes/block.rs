use crate::core::layout::elements::RectElement;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{AnchorLocation, LayoutBuffer, LayoutEnvironment, LayoutNode, LayoutResult};
use crate::core::layout::nodes::page_break::PageBreakNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::border::Border;
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;
use crate::core::idf::IRNode;

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
        let children = engine.build_layout_node_children(ir_children, style.clone());
        Self { id, children, style }
    }
}

impl LayoutNode for BlockNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        // 1. Determine horizontal space availability
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);
        let padding_x = self.style.padding.left + self.style.padding.right;
        let margin_x = 0.0; // Margins don't reduce content box in measurement logic usually, but affect bounding box.
        // Standard block model: width applies to content.

        // Calculate the width available for children
        // If parent has bounded width, we pass that down minus our own spacing
        let child_constraints = if constraints.has_bounded_width() {
            let content_width_limit = (constraints.max_width
                - padding_x
                - border_left_width
                - border_right_width).max(0.0);

            BoxConstraints {
                min_width: 0.0,
                max_width: content_width_limit,
                min_height: 0.0,
                max_height: f32::INFINITY
            }
        } else {
            // Unbounded (intrinsic measurement)
            BoxConstraints {
                min_width: 0.0,
                max_width: f32::INFINITY,
                min_height: 0.0,
                max_height: f32::INFINITY
            }
        };

        // 2. Measure children
        let mut max_child_width: f32 = 0.0;
        let mut total_content_height: f32 = 0.0;

        for child in &mut self.children {
            let child_size = child.measure(env, child_constraints);
            max_child_width = max_child_width.max(child_size.width);
            total_content_height += child_size.height;
        }

        // 3. Calculate own dimensions
        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let padding_y = self.style.padding.top + self.style.padding.bottom;
        let margin_y = self.style.margin.top + self.style.margin.bottom;

        let height = if let Some(Dimension::Pt(h)) = self.style.height {
            margin_y + h
        } else {
            margin_y
                + border_top_width
                + padding_y
                + total_content_height
                + border_bottom_width
        };

        // For width:
        // If we are constrained tightly (e.g. "fill available"), we take max_width.
        // If we are loose/unbounded (e.g. intrinsic), we take the content width + spacing.
        let computed_width = if constraints.has_bounded_width() {
            constraints.max_width // Blocks fill width
        } else {
            max_child_width + padding_x + border_left_width + border_right_width
        };

        Size::new(computed_width, height)
    }

    fn layout(&mut self, env: &LayoutEnvironment, buf: &mut LayoutBuffer) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: env.local_page_index,
                y_pos: buf.cursor.1 + buf.bounds.y,
            };
            buf.defined_anchors.insert(id.clone(), location);
        }

        // --- Vertical Margin Collapsing ---
        let margin_to_add = self.style.margin.top.max(buf.last_v_margin);
        if !buf.is_empty() && margin_to_add > buf.available_height() {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }
        buf.advance_cursor(margin_to_add);
        buf.last_v_margin = 0.0;

        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        let block_start_y_in_ctx = buf.cursor.1;
        buf.advance_cursor(border_top_width + self.style.padding.top);
        let content_start_y_in_ctx = buf.cursor.1;

        let child_bounds = geom::Rect {
            x: buf.bounds.x + border_left_width + self.style.padding.left,
            y: buf.bounds.y + content_start_y_in_ctx,
            width: buf.bounds.width
                - self.style.padding.left
                - self.style.padding.right
                - border_left_width
                - border_right_width,
            height: buf.available_height(),
        };
        let mut child_buf = LayoutBuffer {
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: &mut *buf.elements,
            last_v_margin: 0.0,
            defined_anchors: &mut *buf.defined_anchors,
            index_entries: &mut *buf.index_entries,
        };

        for (i, child) in self.children.iter_mut().enumerate() {
            match child.layout(env, &mut child_buf) {
                Ok(LayoutResult::Full) => continue,
                Ok(LayoutResult::Partial(remainder)) => {
                    let content_height = child_buf.cursor.1;

                    draw_background_and_borders(
                        child_buf.elements,
                        buf.bounds,
                        &self.style,
                        block_start_y_in_ctx,
                        content_height
                    );

                    buf.cursor.1 = content_start_y_in_ctx + content_height + self.style.padding.bottom + border_bottom_width;
                    buf.last_v_margin = child_buf.last_v_margin;

                    let mut remaining_children = vec![remainder];
                    remaining_children.extend(self.children.drain((i + 1)..));

                    let mut next_page_block = Box::new(BlockNode {
                        id: self.id.clone(),
                        children: remaining_children,
                        style: self.style.clone(),
                    });
                    // We must measure the remainder to ensure it has correct size properties for the next page
                    // We use tight width because it's continuing a block that fills width
                    next_page_block.measure(env, BoxConstraints::tight_width(buf.bounds.width));

                    return Ok(LayoutResult::Partial(next_page_block));
                }
                Err(e) => {
                    log::warn!("Skipping child element that failed to lay out: {}", e);
                    continue;
                }
            }
        }

        let content_height = child_buf.cursor.1;
        draw_background_and_borders(
            child_buf.elements,
            buf.bounds,
            &self.style,
            block_start_y_in_ctx,
            content_height
        );

        buf.cursor.1 = content_start_y_in_ctx + content_height + self.style.padding.bottom + border_bottom_width;
        buf.last_v_margin = self.style.margin.bottom.max(child_buf.last_v_margin);

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
    elements: &mut Vec<PositionedElement>,
    bounds: geom::Rect,
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

    // Helper to push relative to the bounds provided
    let mut push_element = |mut element: PositionedElement, x: f32, y: f32| {
        element.x += bounds.x + x;
        element.y += bounds.y + y;
        elements.push(element);
    };

    // Draw background
    if style.background_color.is_some() {
        let bg_style = Arc::new(ComputedStyle {
            background_color: style.background_color.clone(),
            ..ComputedStyle::default()
        });
        let bg_rect = geom::Rect {
            x: border_left_width,
            y: border_top_width,
            width: bounds.width - border_left_width - border_right_width,
            height: inner_height,
        };
        let bg = PositionedElement {
            element: LayoutElement::Rectangle(RectElement),
            style: bg_style,
            ..PositionedElement::from_rect(bg_rect)
        };
        push_element(bg, 0.0, start_y);
    }

    let mut draw_border = |b: &Option<Border>, rect: geom::Rect| {
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
                push_element(positioned_rect, 0.0, start_y);
            }
        }
    };

    let bounds_width = bounds.width;
    draw_border(&style.border_top, geom::Rect { x: 0.0, y: 0.0, width: bounds_width, height: border_top_width });
    draw_border(&style.border_bottom, geom::Rect { x: 0.0, y: total_height - border_bottom_width, width: bounds_width, height: border_bottom_width });
    draw_border(&style.border_left, geom::Rect { x: 0.0, y: 0.0, width: border_left_width, height: total_height });
    draw_border(&style.border_right, geom::Rect { x: bounds_width - border_right_width, y: 0.0, width: border_right_width, height: total_height });
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