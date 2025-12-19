use super::RenderNode;
use crate::LayoutError;
use crate::engine::{LayoutEngine, LayoutStore};
use crate::interface::{
    BlockState, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState,
};
use crate::painting::box_painter::create_background_and_borders;
use crate::style::ComputedStyle;
use petty_idf::{IRNode, TextStr};
use petty_style::dimension::Dimension;
use petty_types::geometry::{self, BoxConstraints, Size};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BlockNode<'a> {
    pub id: Option<&'a str>,
    pub children: &'a [RenderNode<'a>],
    pub style: Arc<ComputedStyle>,
}

impl<'a> BlockNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let (id_string, children_ir, style) = match node {
            IRNode::Block { meta, children } => {
                let style = engine.compute_style(
                    &meta.style_sets,
                    meta.style_override.as_ref(),
                    &parent_style,
                );
                (&meta.id, children, style)
            }
            IRNode::Root(children) => {
                let style = engine.get_default_style();
                (&None, children, style)
            }
            _ => return Err(LayoutError::BuilderMismatch("Block", node.kind())),
        };

        let child_vec = engine.build_layout_node_children(children_ir, style.clone(), store)?;
        let children = store.bump.alloc_slice_copy(&child_vec);

        let id = id_string.as_ref().map(|s| store.alloc_str(s));

        let style_ref = store.cache_style(style);

        let node = store.bump.alloc(Self {
            id,
            children,
            style: style_ref,
        });

        Ok(RenderNode::Block(node))
    }

    pub fn new_from_children(
        id_string: Option<TextStr>,
        children_vec: Vec<RenderNode<'a>>,
        style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Self {
        let style_ref = store.cache_style(style);
        Self {
            id: id_string.as_ref().map(|s| store.alloc_str(s)),
            children: store.bump.alloc_slice_copy(&children_vec),
            style: style_ref,
        }
    }
}

impl<'a> LayoutNode for BlockNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style.as_ref()
    }

    fn measure(
        &self,
        env: &LayoutEnvironment,
        constraints: BoxConstraints,
    ) -> Result<Size, LayoutError> {
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let padding_y = self.style.padding_y();
        let border_y = self.style.border_y();
        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        if let (Some(Dimension::Pt(w)), Some(Dimension::Pt(h))) =
            (&self.style.box_model.width, &self.style.box_model.height)
        {
            return Ok(Size::new(w + h_deduction, h + margin_y));
        }

        let child_constraints = self.style.content_constraints(constraints);

        let mut max_child_width: f32 = 0.0;
        let mut total_content_height: f32 = 0.0;

        for child in self.children {
            let child_size = child.measure(env, child_constraints)?;
            max_child_width = max_child_width.max(child_size.width);
            total_content_height += child_size.height;
        }

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

        Ok(Size::new(computed_width, height))
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = self.id {
            ctx.register_anchor(id);
        }

        let (start_index, mut child_resume_state) = if let Some(state) = break_state {
            let block_state = state.as_block()?;
            (block_state.child_index, block_state.child_state.map(|b| *b))
        } else {
            (0, None)
        };

        let is_continuation = start_index > 0 || child_resume_state.is_some();

        // Use LayoutContext helpers for margin collapsing
        if !is_continuation {
            if ctx.prepare_for_block(self.style.box_model.margin.top) {
                return Ok(LayoutResult::Break(NodeState::Block(BlockState {
                    child_index: 0,
                    child_state: None,
                })));
            }
        } else {
            // If continuing, ensure previous margins are cleared so we don't double add them
            ctx.last_v_margin = 0.0;
        }

        let border_top = self.style.border_top_width();
        let border_left = self.style.border_left_width();

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

        let child_bounds = geometry::Rect {
            x: ctx_bounds.x + border_left + self.style.box_model.padding.left,
            y: ctx_bounds.y + content_start_y_in_ctx,
            width: ctx_bounds.width - self.style.padding_x() - self.style.border_x(),
            height: ctx.available_height(),
        };

        let mut child_ctx = ctx.child(child_bounds);
        let mut split_res = LayoutResult::Finished;
        for (i, child) in self.children.iter().enumerate().skip(start_index) {
            let resume = if i == start_index {
                child_resume_state.take()
            } else {
                None
            };

            let res = child.layout(&mut child_ctx, child_constraints, resume)?;

            match res {
                LayoutResult::Finished => {}
                LayoutResult::Break(next_state) => {
                    split_res = LayoutResult::Break(NodeState::Block(BlockState {
                        child_index: i,
                        child_state: Some(Box::new(next_state)),
                    }));
                    break;
                }
            }
        }
        let child_cursor_y = child_ctx.cursor_y();
        let actual_used_height = child_cursor_y;

        let bg_elements = create_background_and_borders(
            ctx.bounds(),
            &self.style,
            block_start_y_in_ctx,
            actual_used_height,
            !is_continuation,
            matches!(split_res, LayoutResult::Finished),
        );

        for el in bg_elements {
            // Background elements are already positioned relative to the block bounds
            // (block_start_y_in_ctx is relative to bounds).
            // We use push_element_at(..., 0.0, 0.0) to avoid double-adding ctx.cursor.
            ctx.push_element_at(el, 0.0, 0.0);
        }

        match split_res {
            LayoutResult::Finished => {
                let border_bottom = self.style.border_bottom_width();
                let bottom_spacing = self.style.box_model.padding.bottom + border_bottom;
                ctx.set_cursor_y(content_start_y_in_ctx + actual_used_height + bottom_spacing);
                // Use finish_block helper
                ctx.finish_block(self.style.box_model.margin.bottom);
                Ok(LayoutResult::Finished)
            }
            LayoutResult::Break(state) => {
                ctx.set_cursor_y(content_start_y_in_ctx + actual_used_height);
                Ok(LayoutResult::Break(state))
            }
        }
    }

    fn check_for_page_break(&self) -> Option<Option<TextStr>> {
        if let Some(first_child) = self.children.first() {
            first_child.check_for_page_break()
        } else {
            None
        }
    }
}
