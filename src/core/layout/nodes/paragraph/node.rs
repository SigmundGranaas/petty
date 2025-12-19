use crate::core::idf::TextStr;
use crate::core::layout::nodes::RenderNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{
    LayoutContext, LayoutEnvironment, LayoutError, LayoutNode, LayoutResult,
    NodeState, ParagraphState
};
// Use explicit geometry types from base to match Trait definition
use crate::core::base::geometry::{BoxConstraints, Size};
use crate::core::layout::text::builder::{InlineImageEntry, TextSpan};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

use super::layout::ParagraphLayout;

#[derive(Debug)]
pub struct ParagraphNode<'a> {
    pub unique_id: usize,
    pub id: Option<&'a str>,
    pub spans: &'a [TextSpan<'a>],
    pub full_text: &'a str,
    pub links: &'a [&'a str],
    pub inline_images: &'a [InlineImageEntry<'a>],
    pub style: Arc<ComputedStyle>,
}

impl<'a> LayoutNode for ParagraphNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style.as_ref()
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Result<Size, LayoutError> {
        let max_width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            f32::INFINITY
        };

        let shaped_runs = self.resolve_shaping(env);
        let layout = self.resolve_layout(env, &shaped_runs, max_width);

        Ok(self.resolve_size(&layout, constraints))
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

        let scroll_offset = if let Some(state) = break_state {
            state.as_paragraph()?.scroll_offset
        } else {
            0.0
        };

        let is_continuation = scroll_offset > 0.0;

        if !is_continuation {
            if ctx.prepare_for_block(self.style.box_model.margin.top) {
                return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                    scroll_offset: 0.0,
                })));
            }
        } else {
            ctx.last_v_margin = 0.0;
        }

        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            ctx.bounds().width
        };

        let shaped_runs = self.resolve_shaping(&ctx.env);
        let layout = self.resolve_layout(&ctx.env, &shaped_runs, width);

        self.render_lines_to_context(ctx, &layout, scroll_offset)
    }
}

impl<'a> ParagraphNode<'a> {
    fn resolve_size(&self, layout: &ParagraphLayout, constraints: BoxConstraints) -> Size {
        let mut width = layout.max_line_width;
        let mut height = layout.total_height;

        if let Some(Dimension::Pt(w)) = self.style.box_model.width {
            width = w;
        } else if constraints.is_tight() {
            width = constraints.max_width;
        }

        if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            height = h;
        }

        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;
        Size::new(width, height + margin_y)
    }
}