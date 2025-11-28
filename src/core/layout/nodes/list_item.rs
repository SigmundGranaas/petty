// src/core/layout/nodes/list_item.rs

use crate::core::idf::{IRNode, InlineNode, TextStr};
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, ListItemState, NodeState, RenderNode,
};
use crate::core::layout::nodes::block::create_background_and_borders;
use crate::core::layout::nodes::list_utils::get_marker_text;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{
    LayoutElement, LayoutEngine, LayoutError, PositionedElement, TextElement,
};
use crate::core::style::dimension::Dimension;
use crate::core::style::list::ListStylePosition;
use crate::core::style::text::TextDecoration;
use bumpalo::Bump;
use std::sync::Arc;

#[derive(Debug)]
pub struct ListItemNode<'a> {
    id: Option<TextStr>,
    children: &'a [RenderNode<'a>],
    style: Arc<ComputedStyle>,
    marker_text: String,
}

impl<'a> ListItemNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        // ListItems are usually built by ListNode, but for standalone cases:
        let item = Self::new(node, engine, parent_style, 1, 0, arena)?;
        Ok(RenderNode::ListItem(arena.alloc(item)))
    }

    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        index: usize,
        depth: usize,
        arena: &'a Bump,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let IRNode::ListItem {
            meta,
            children: ir_children,
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("ListItem", node.kind()));
        };

        let marker_text = get_marker_text(&style, index, depth);

        let mut children_to_process = ir_children.clone();
        if style.list.style_position == ListStylePosition::Inside && !marker_text.is_empty() {
            if let Some(first_child) = children_to_process.first_mut() {
                if let IRNode::Paragraph { children, .. } = first_child {
                    let prefix = format!("{} ", marker_text);
                    children.insert(0, InlineNode::Text(prefix.into()));
                }
            }
        }

        let mut children_vec = Vec::new();
        for child_ir in &children_to_process {
            if let IRNode::List { .. } = child_ir {
                let list = super::list::ListNode::new_with_depth(child_ir, engine, style.clone(), depth + 1, arena)?;
                children_vec.push(RenderNode::List(arena.alloc(list)));
            } else {
                children_vec.push(engine.build_layout_node_tree(child_ir, style.clone(), arena)?);
            }
        }

        Ok(Self {
            id: meta.id.clone(),
            children: arena.alloc_slice_copy(&children_vec),
            style,
            marker_text,
        })
    }
}

impl<'a> LayoutNode for ListItemNode<'a> {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let border_left = self.style.border_left_width();
        let border_right = self.style.border_right_width();
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;
        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            measure_text_using_env(env, &self.marker_text, &self.style)
                + self.style.text.font_size * MARKER_SPACING_FACTOR
        } else {
            0.0
        };

        let child_constraints = if constraints.has_bounded_width() {
            let w = (constraints.max_width
                - self.style.box_model.padding.left
                - self.style.box_model.padding.right
                - border_left
                - border_right
                - indent)
                .max(0.0);
            BoxConstraints {
                min_width: 0.0,
                max_width: w,
                min_height: 0.0,
                max_height: f32::INFINITY,
            }
        } else {
            BoxConstraints::default()
        };

        let mut total_content_height = 0.0;
        for child in self.children {
            total_content_height += child.measure(env, child_constraints).height;
        }

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();

        let height = if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            h
        } else {
            border_top
                + self.style.box_model.padding.top
                + total_content_height
                + self.style.box_model.padding.bottom
                + border_bottom
        };

        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            0.0
        };

        Size::new(width, height)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        _constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            ctx.register_anchor(id);
        }

        let (start_index, mut child_resume_state) = if let Some(state) = break_state {
            let list_state = state.as_list_item()?;
            (
                list_state.child_index,
                list_state.child_state.map(|b| *b),
            )
        } else {
            (0, None)
        };
        let is_continuation = start_index > 0 || child_resume_state.is_some();

        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;

        let block_start_y_in_ctx = ctx.cursor_y();

        // Measure marker using context's font system
        let marker_width = measure_text_using_ctx(ctx, &self.marker_text, &self.style);

        if !self.marker_text.is_empty() && !is_continuation {
            let should_draw = is_outside_marker;

            if should_draw {
                let marker_available_height = self.style.text.line_height;
                if marker_available_height > ctx.available_height() && !ctx.is_empty() {
                    return Ok(LayoutResult::Break(NodeState::ListItem(ListItemState {
                        child_index: 0,
                        child_state: None,
                    })));
                }

                let marker_box = PositionedElement {
                    x: 0.0,
                    y: self.style.border_top_width() + self.style.box_model.padding.top,
                    width: marker_width,
                    height: self.style.text.line_height,
                    element: LayoutElement::Text(TextElement {
                        content: self.marker_text.clone(),
                        href: None,
                        text_decoration: TextDecoration::None,
                    }),
                    style: self.style.clone(),
                };
                ctx.push_element_at(marker_box, 0.0, block_start_y_in_ctx);
            }
        }

        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            marker_width + self.style.text.font_size * MARKER_SPACING_FACTOR
        } else {
            0.0
        };

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();
        let border_left = self.style.border_left_width();

        if !is_continuation {
            ctx.advance_cursor(border_top + self.style.box_model.padding.top);
        }
        let content_start_y_in_ctx = ctx.cursor_y();

        let ctx_bounds = ctx.bounds();
        let child_bounds = geom::Rect {
            x: ctx_bounds.x + border_left + self.style.box_model.padding.left + indent,
            y: ctx_bounds.y + content_start_y_in_ctx,
            width: ctx_bounds.width - self.style.padding_x() - self.style.border_x() - indent,
            height: ctx.available_height(),
        };

        let mut child_split_result = LayoutResult::Finished;

        let _ = ctx.with_child_bounds(child_bounds, |child_ctx| {
            for (i, child) in self.children.iter().enumerate().skip(start_index) {
                let child_constraints = BoxConstraints::tight_width(child_bounds.width);

                let res = child.layout(
                    child_ctx,
                    child_constraints,
                    child_resume_state.take(),
                )?;

                match res {
                    LayoutResult::Finished => {}
                    LayoutResult::Break(next_state) => {
                        child_split_result = LayoutResult::Break(NodeState::ListItem(ListItemState {
                            child_index: i,
                            child_state: Some(Box::new(next_state)),
                        }));
                        return Ok(());
                    }
                }
            }
            Ok(())
        })?;

        // Recalculate used height based on split result
        let used_height = if matches!(child_split_result, LayoutResult::Finished) {
            ctx.cursor_y() - content_start_y_in_ctx
        } else {
            ctx.available_height()
        };

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
                ctx.set_cursor_y(
                    content_start_y_in_ctx
                        + used_height
                        + self.style.box_model.padding.bottom
                        + border_bottom,
                );
                Ok(LayoutResult::Finished)
            }
            LayoutResult::Break(state) => {
                ctx.set_cursor_y(content_start_y_in_ctx + used_height);
                Ok(LayoutResult::Break(state))
            }
        }
    }
}

// Helpers to measure text using environment/context without borrowing engine
fn measure_text_using_env(env: &mut LayoutEnvironment, text: &str, style: &Arc<ComputedStyle>) -> f32 {
    let mut buffer = cosmic_text::Buffer::new(env.font_system, cosmic_text::Metrics::new(style.text.font_size, style.text.line_height));
    let attrs = env.engine.font_manager.attrs_from_style(style);
    buffer.set_text(env.font_system, text, &attrs, cosmic_text::Shaping::Advanced);
    buffer.shape_until_scroll(env.font_system, false);
    buffer.layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
}

fn measure_text_using_ctx(ctx: &mut LayoutContext, text: &str, style: &Arc<ComputedStyle>) -> f32 {
    let mut buffer = cosmic_text::Buffer::new(ctx.font_system, cosmic_text::Metrics::new(style.text.font_size, style.text.line_height));
    let attrs = ctx.engine.font_manager.attrs_from_style(style);
    buffer.set_text(ctx.font_system, text, &attrs, cosmic_text::Shaping::Advanced);
    buffer.shape_until_scroll(ctx.font_system, false);
    buffer.layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
}