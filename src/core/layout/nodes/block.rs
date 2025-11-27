// src/core/layout/nodes/block.rs

use crate::core::idf::IRNode;
use crate::core::layout::elements::RectElement;
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::node::{
    BlockState, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::border::Border;
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

#[derive(Debug)]
pub struct BlockNode {
    id: Option<String>,
    children: Vec<RenderNode>,
    style: Arc<ComputedStyle>,
}

impl BlockNode {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        let (id, children_ir, style) = match node {
            IRNode::Block { meta, children } => {
                let style = engine.compute_style(
                    &meta.style_sets,
                    meta.style_override.as_ref(),
                    &parent_style,
                );
                (meta.id.clone(), children, style)
            }
            IRNode::ListItem { meta, children } => {
                let style = engine.compute_style(
                    &meta.style_sets,
                    meta.style_override.as_ref(),
                    &parent_style,
                );
                (meta.id.clone(), children, style)
            }
            IRNode::Root(children) => {
                let style = engine.get_default_style();
                (None, children, style)
            }
            _ => return Err(LayoutError::BuilderMismatch("Block", node.kind())),
        };

        let children_nodes = engine.build_layout_node_children(children_ir, style.clone())?;
        Ok(RenderNode::Block(Box::new(Self {
            id,
            children: children_nodes,
            style,
        })))
    }

    pub fn new_from_children(
        id: Option<String>,
        children: Vec<RenderNode>,
        style: Arc<ComputedStyle>,
    ) -> Self {
        Self {
            id,
            children,
            style,
        }
    }
}

impl LayoutNode for BlockNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let child_constraints = self.style.content_constraints(constraints);

        let mut max_child_width: f32 = 0.0;
        let mut total_content_height: f32 = 0.0;

        for child in &self.children {
            let child_size = child.measure(env, child_constraints);
            max_child_width = max_child_width.max(child_size.width);
            total_content_height += child_size.height;
        }

        let padding_y = self.style.padding_y();
        let border_y = self.style.border_y();
        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        let height = if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            margin_y + h
        } else {
            margin_y + border_y + padding_y + total_content_height
        };

        let computed_width = if constraints.has_bounded_width() {
            constraints.max_width
        } else if let Some(Dimension::Pt(w)) = self.style.box_model.width {
            w + h_deduction
        } else {
            max_child_width + h_deduction
        };

        Size::new(computed_width, height)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            ctx.register_anchor(id);
        }

        // 1. Resolve State using type-safe enum
        let (start_index, mut child_resume_state) = if let Some(state) = break_state {
            let block_state = state.as_block()?;
            (
                block_state.child_index,
                block_state.child_state.map(|b| *b),
            )
        } else {
            (0, None)
        };

        let is_continuation = start_index > 0 || child_resume_state.is_some();

        // 2. Margins
        if !is_continuation {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
            // If margin pushes us over and we aren't at top, partial
            if ctx.cursor_y() > 0.0 && margin_to_add > ctx.available_height() {
                return Ok(LayoutResult::Break(NodeState::Block(BlockState {
                    child_index: 0,
                    child_state: None,
                })));
            }
            ctx.advance_cursor(margin_to_add);
        }
        ctx.last_v_margin = 0.0;

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();
        let border_left = self.style.border_left_width();

        // Borders/Padding only on start/end
        let top_spacing = if !is_continuation {
            border_top + self.style.box_model.padding.top
        } else {
            0.0
        };

        let block_start_y_in_ctx = ctx.cursor_y();
        ctx.advance_cursor(top_spacing);
        let content_start_y_in_ctx = ctx.cursor_y();

        let child_constraints = self.style.content_constraints(constraints);
        let ctx_bounds = ctx.bounds();

        let child_bounds = geom::Rect {
            x: ctx_bounds.x + border_left + self.style.box_model.padding.left,
            y: ctx_bounds.y + content_start_y_in_ctx,
            width: ctx_bounds.width - self.style.padding_x() - self.style.border_x(),
            height: ctx.available_height(),
        };

        let mut child_split_result = LayoutResult::Finished;

        // 3. Child Iteration
        let used_height = ctx.with_child_bounds(child_bounds, |child_ctx| {
            for (i, child) in self.children.iter().enumerate().skip(start_index) {
                // Take resume state if it belongs to this child
                let resume = if i == start_index {
                    child_resume_state.take()
                } else {
                    None
                };

                let Ok(res) = child.layout(child_ctx, child_constraints, resume) else {
                    return Err(LayoutError::Generic(format!(
                        "Layout failed for child index {}",
                        i
                    )));
                };

                match res {
                    LayoutResult::Finished => {}
                    LayoutResult::Break(next_state) => {
                        child_split_result = LayoutResult::Break(NodeState::Block(BlockState {
                            child_index: i,
                            child_state: Some(Box::new(next_state)),
                        }));
                        break;
                    }
                }
            }
            Ok(child_ctx.cursor_y())
        })?;

        let bg_elements = create_background_and_borders(
            ctx.bounds(),
            &self.style,
            block_start_y_in_ctx,
            used_height,
            !is_continuation,
            matches!(child_split_result, LayoutResult::Finished),
        );

        for el in bg_elements {
            ctx.push_element(el);
        }

        match child_split_result {
            LayoutResult::Finished => {
                let bottom_spacing = self.style.box_model.padding.bottom + border_bottom;
                ctx.set_cursor_y(content_start_y_in_ctx + used_height + bottom_spacing);
                ctx.last_v_margin = self.style.box_model.margin.bottom;
                Ok(LayoutResult::Finished)
            }
            LayoutResult::Break(state) => {
                ctx.set_cursor_y(content_start_y_in_ctx + used_height);
                Ok(LayoutResult::Break(state))
            }
        }
    }

    fn check_for_page_break(&self) -> Option<Option<String>> {
        if let Some(first_child) = self.children.first() {
            if let Some(res) = first_child.check_for_page_break() {
                return Some(res);
            }
        }
        None
    }
}

pub fn create_background_and_borders(
    bounds: geom::Rect,
    style: &Arc<ComputedStyle>,
    start_y: f32,
    content_height: f32,
    draw_top: bool,
    draw_bottom: bool,
) -> Vec<PositionedElement> {
    let mut elements = Vec::new();

    let border_top = if draw_top {
        style.border_top_width()
    } else {
        0.0
    };
    let border_bottom = if draw_bottom {
        style.border_bottom_width()
    } else {
        0.0
    };
    let border_left = style.border_left_width();
    let border_right = style.border_right_width();

    let padding_top = if draw_top {
        style.box_model.padding.top
    } else {
        0.0
    };
    let padding_bottom = if draw_bottom {
        style.box_model.padding.bottom
    } else {
        0.0
    };

    let total_height = border_top + padding_top + content_height + padding_bottom + border_bottom;

    if total_height <= 0.0 {
        return elements;
    }

    // Helper to push elements relative to the bounds origin
    let mut push = |mut element: PositionedElement, x: f32, y: f32| {
        element.x += x;
        element.y += y;
        elements.push(element);
    };

    if style.misc.background_color.is_some() {
        let mut bg_style = ComputedStyle::default();
        bg_style.misc.background_color = style.misc.background_color.clone();

        let bg_rect = geom::Rect {
            x: border_left,
            y: border_top,
            width: bounds.width - border_left - border_right,
            height: total_height - border_top - border_bottom,
        };
        let bg = PositionedElement {
            element: LayoutElement::Rectangle(RectElement),
            style: Arc::new(bg_style),
            ..PositionedElement::from_rect(bg_rect)
        };
        push(bg, 0.0, start_y);
    }

    let bounds_width = bounds.width;

    let mut draw_border = |b: &Option<Border>, rect: geom::Rect| {
        if let Some(border) = b {
            if border.width > 0.0 {
                let mut border_style = ComputedStyle::default();
                border_style.misc.background_color = Some(border.color.clone());
                let positioned_rect = PositionedElement {
                    element: LayoutElement::Rectangle(RectElement),
                    style: Arc::new(border_style),
                    ..PositionedElement::from_rect(rect)
                };
                push(positioned_rect, 0.0, start_y);
            }
        }
    };

    if draw_top {
        draw_border(
            &style.border.top,
            geom::Rect {
                x: 0.0,
                y: 0.0,
                width: bounds_width,
                height: border_top,
            },
        );
    }
    if draw_bottom {
        draw_border(
            &style.border.bottom,
            geom::Rect {
                x: 0.0,
                y: total_height - border_bottom,
                width: bounds_width,
                height: border_bottom,
            },
        );
    }

    draw_border(
        &style.border.left,
        geom::Rect {
            x: 0.0,
            y: 0.0,
            width: border_left,
            height: total_height,
        },
    );
    draw_border(
        &style.border.right,
        geom::Rect {
            x: bounds_width - border_right,
            y: 0.0,
            width: border_right,
            height: total_height,
        },
    );

    elements
}