use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutContext, LayoutElement, LayoutEngine, LayoutEnvironment, LayoutError, PositionedElement, TextElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::list::ListStylePosition;
use crate::core::style::text::TextDecoration;
use std::sync::Arc;
use crate::core::idf::{IRNode, InlineNode};
use crate::core::layout::geom::{BoxConstraints, Size};
use std::any::Any;
use crate::core::layout::node::{LayoutNode, LayoutResult, RenderNode};
use crate::core::layout::nodes::block::create_background_and_borders;
use crate::core::layout::nodes::list_utils::get_marker_text;

#[derive(Debug)]
pub struct ListItemNode {
    id: Option<String>,
    children: Vec<RenderNode>,
    style: Arc<ComputedStyle>,
    marker_text: String,
}

#[derive(Debug)]
struct ListItemState {
    child_index: usize,
    child_state: Option<Box<dyn Any + Send>>,
}

impl ListItemNode {
    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        index: usize,
        depth: usize,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (meta, ir_children) = match node {
            IRNode::ListItem { meta, children } => (meta, children),
            _ => return Err(LayoutError::BuilderMismatch("ListItem", node.kind())),
        };

        let marker_text = get_marker_text(&style, index, depth);

        // For 'Inside' positioning, the marker acts like inline text at the start of the content.
        // We inject it into the first paragraph child if applicable.
        let mut children_to_process = ir_children.clone();
        if style.list.style_position == ListStylePosition::Inside && !marker_text.is_empty() {
            if let Some(first_child) = children_to_process.first_mut() {
                if let IRNode::Paragraph { children, .. } = first_child {
                    let prefix = format!("{} ", marker_text);
                    children.insert(0, InlineNode::Text(prefix));
                }
            }
        }

        let mut children: Vec<RenderNode> = Vec::new();
        for child_ir in &children_to_process {
            if let IRNode::List { .. } = child_ir {
                children.push(Box::new(super::list::ListNode::new_with_depth(
                    child_ir,
                    engine,
                    style.clone(),
                    depth + 1,
                )?));
            } else {
                children.push(engine.build_layout_node_tree(child_ir, style.clone())?);
            }
        }

        Ok(Self {
            id: meta.id.clone(),
            children,
            style,
            marker_text,
        })
    }
}

impl LayoutNode for ListItemNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let border_left = self.style.border_left_width();
        let border_right = self.style.border_right_width();
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;
        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            env.engine.measure_text_width(&self.marker_text, &self.style) + self.style.text.font_size * MARKER_SPACING_FACTOR
        } else {
            0.0
        };

        let child_constraints = if constraints.has_bounded_width() {
            let w = (constraints.max_width
                - self.style.box_model.padding.left
                - self.style.box_model.padding.right
                - border_left
                - border_right
                - indent).max(0.0);
            BoxConstraints {
                min_width: 0.0, max_width: w,
                min_height: 0.0, max_height: f32::INFINITY
            }
        } else {
            BoxConstraints {
                min_width: 0.0, max_width: f32::INFINITY,
                min_height: 0.0, max_height: f32::INFINITY
            }
        };

        let mut total_content_height = 0.0;
        for child in &self.children {
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

        let width = if constraints.has_bounded_width() { constraints.max_width } else { 0.0 };

        Size::new(width, height)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        _constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>
    ) -> Result<LayoutResult, LayoutError> {

        let (start_index, mut child_break_state) = if let Some(state) = break_state {
            let s = *state.downcast::<ListItemState>().map_err(|_| LayoutError::Generic("Invalid state for ListItemNode".into()))?;
            (s.child_index, s.child_state)
        } else {
            (0, None)
        };
        let is_continuation = start_index > 0 || child_break_state.is_some();

        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;

        let block_start_y_in_ctx = ctx.cursor.1;
        let marker_width = ctx.engine.measure_text_width(&self.marker_text, &self.style);

        // Draw marker if applicable
        if !self.marker_text.is_empty() && !is_continuation {
            // FIX: Only separate element for Outside.
            // Satisfies tests expecting 1 element for Inside.
            let should_draw = is_outside_marker;

            if should_draw {
                let marker_available_height = self.style.text.line_height;
                if marker_available_height > ctx.available_height() && !ctx.is_empty() {
                    return Ok(LayoutResult::Break(Box::new(ListItemState { child_index: 0, child_state: None })));
                }

                let marker_box = PositionedElement {
                    x: 0.0, // Always start at 0 local x
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
        let content_start_y_in_ctx = ctx.cursor.1;

        let child_bounds = geom::Rect {
            x: ctx.bounds.x + border_left + self.style.box_model.padding.left + indent,
            y: ctx.bounds.y + content_start_y_in_ctx,
            width: ctx.bounds.width - self.style.padding_x() - self.style.border_x() - indent,
            height: ctx.available_height(),
        };

        let mut child_split_result = LayoutResult::Finished;

        let used_height = ctx.with_child_bounds(child_bounds, |child_ctx| {
            for (i, child) in self.children.iter().enumerate().skip(start_index) {
                // Determine constraints for children. Usually width is tight to bounds.
                let child_constraints = BoxConstraints::tight_width(child_bounds.width);

                let res = child.layout(child_ctx, child_constraints, child_break_state.take())?;

                match res {
                    LayoutResult::Finished => {}
                    LayoutResult::Break(next_state) => {
                        child_split_result = LayoutResult::Break(Box::new(ListItemState {
                            child_index: i,
                            child_state: Some(next_state),
                        }));
                        break;
                    }
                }
            }
            Ok(child_ctx.cursor.1)
        })?;

        create_background_and_borders(
            ctx.bounds,
            &self.style,
            block_start_y_in_ctx,
            used_height,
            !is_continuation,
            matches!(child_split_result, LayoutResult::Finished)
        );

        match child_split_result {
            LayoutResult::Finished => {
                ctx.cursor.1 = content_start_y_in_ctx + used_height + self.style.box_model.padding.bottom + border_bottom;
                Ok(LayoutResult::Finished)
            }
            LayoutResult::Break(state) => {
                ctx.cursor.1 = content_start_y_in_ctx + used_height;
                Ok(LayoutResult::Break(state))
            }
        }
    }
}