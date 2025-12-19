use super::RenderNode;
use crate::engine::{LayoutEngine, LayoutStore};
use crate::interface::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, ListItemState, NodeState,
};
use crate::nodes::list_utils::get_marker_text;
use crate::painting::box_painter::create_background_and_borders;
use crate::style::ComputedStyle;
use crate::{LayoutElement, LayoutError, PositionedElement, TextElement};
use petty_idf::{IRNode, InlineNode, TextStr};
use petty_style::dimension::Dimension;
use petty_style::list::ListStylePosition;
use petty_style::text::TextDecoration;
use petty_types::geometry::{self, BoxConstraints, Size};
use std::sync::Arc;

#[derive(Debug)]
pub struct ListItemNode<'a> {
    id: Option<TextStr>,
    children: &'a [RenderNode<'a>],
    style: Arc<ComputedStyle>,
    marker_text: &'a str,
}

impl<'a> ListItemNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        // Default start 1, depth 0 if built directly (unlikely)
        let item = ListItemNode::new(node, engine, parent_style, 1, 0, store)?;
        Ok(RenderNode::ListItem(store.bump.alloc(item)))
    }

    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        index: usize,
        depth: usize,
        store: &'a LayoutStore,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let IRNode::ListItem {
            meta,
            children: ir_children,
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("ListItem", node.kind()));
        };

        let marker_string = get_marker_text(&style, index, depth);
        let marker_text = store.alloc_str(&marker_string);

        // For "Inside" positioning, modify the first paragraph to include the marker
        let mut children_to_process = ir_children.clone();
        if style.list.style_position == ListStylePosition::Inside && !marker_text.is_empty() {
            if let Some(first_child) = children_to_process.first_mut() {
                if let IRNode::Paragraph { children, .. } = first_child {
                    let prefix = format!("{} ", marker_text);
                    children.insert(0, InlineNode::Text(prefix));
                }
            }
        }

        let mut children_vec = Vec::new();
        for child_ir in &children_to_process {
            if let IRNode::List { .. } = child_ir {
                let list = super::list::ListNode::new_with_depth(
                    child_ir,
                    engine,
                    style.clone(),
                    depth + 1,
                    store,
                )?;
                children_vec.push(RenderNode::List(store.bump.alloc(list)));
            } else {
                children_vec.push(engine.build_layout_node_tree(child_ir, style.clone(), store)?);
            }
        }

        let style_ref = store.cache_style(style);

        Ok(Self {
            id: meta.id.clone(),
            children: store.bump.alloc_slice_copy(&children_vec),
            style: style_ref,
            marker_text,
        })
    }
}

impl<'a> LayoutNode for ListItemNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style.as_ref()
    }

    fn measure(
        &self,
        env: &LayoutEnvironment,
        constraints: BoxConstraints,
    ) -> Result<Size, LayoutError> {
        let border_left = self.style.border_left_width();
        let border_right = self.style.border_right_width();
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;

        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            env.engine.measure_text_width(self.marker_text, &self.style)
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
            total_content_height += child.measure(env, child_constraints)?.height;
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
            0.0 // Todo: calc min width
        };

        Ok(Size::new(width, height))
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
            (list_state.child_index, list_state.child_state.map(|b| *b))
        } else {
            (0, None)
        };
        let is_continuation = start_index > 0 || child_resume_state.is_some();

        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;

        let block_start_y_in_ctx = ctx.cursor_y();

        let marker_width = ctx
            .env
            .engine
            .measure_text_width(self.marker_text, &self.style);

        // Draw outside marker on the first page only
        if !self.marker_text.is_empty() && !is_continuation && is_outside_marker {
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
                    content: self.marker_text.to_string(),
                    href: None,
                    text_decoration: TextDecoration::None,
                }),
                style: self.style.clone(),
            };
            // Marker is pushed absolute relative to current block start.
            // push_element_at computes absolute position based on args + bounds.
            ctx.push_element_at(marker_box, 0.0, block_start_y_in_ctx);
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
        let child_bounds = geometry::Rect {
            x: ctx_bounds.x + border_left + self.style.box_model.padding.left + indent,
            y: ctx_bounds.y + content_start_y_in_ctx,
            width: ctx_bounds.width - self.style.padding_x() - self.style.border_x() - indent,
            height: ctx.available_height(),
        };

        let mut child_ctx = ctx.child(child_bounds);
        let mut split_res = LayoutResult::Finished;

        for (i, child) in self.children.iter().enumerate().skip(start_index) {
            let child_constraints = BoxConstraints::tight_width(child_bounds.width);

            let res = child.layout(&mut child_ctx, child_constraints, child_resume_state.take())?;

            match res {
                LayoutResult::Finished => {}
                LayoutResult::Break(next_state) => {
                    split_res = LayoutResult::Break(NodeState::ListItem(ListItemState {
                        child_index: i,
                        child_state: Some(Box::new(next_state)),
                    }));
                    break;
                }
            }
        }
        let child_cursor_y = child_ctx.cursor_y();

        let used_height = if matches!(split_res, LayoutResult::Finished) {
            child_cursor_y
        } else {
            ctx.available_height() // Consumed remaining space
        };

        // Delegate background painting to shared logic
        let bg_elements = create_background_and_borders(
            ctx.bounds(),
            &self.style,
            block_start_y_in_ctx,
            used_height,
            !is_continuation,
            matches!(split_res, LayoutResult::Finished),
        );
        for el in bg_elements {
            // Background elements are already absolute-positioned relative to bounds.
            // Use push_element_at(..., 0.0, 0.0) to avoid double-adding ctx.cursor.
            ctx.push_element_at(el, 0.0, 0.0);
        }

        match split_res {
            LayoutResult::Finished => {
                ctx.set_cursor_y(
                    content_start_y_in_ctx
                        + used_height
                        + self.style.box_model.padding.bottom
                        + border_bottom,
                );
                ctx.finish_block(self.style.box_model.margin.bottom);
                Ok(LayoutResult::Finished)
            }
            LayoutResult::Break(state) => {
                ctx.set_cursor_y(content_start_y_in_ctx + used_height);
                Ok(LayoutResult::Break(state))
            }
        }
    }
}
