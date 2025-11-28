// src/core/layout/nodes/page_break.rs

use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use bumpalo::Bump;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PageBreakNode {
    pub master_name: Option<TextStr>,
    style: Arc<ComputedStyle>,
}

impl PageBreakNode {
    pub fn build<'a>(
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let master_name = match node {
            IRNode::PageBreak { master_name } => master_name.clone(),
            _ => return Err(LayoutError::BuilderMismatch("PageBreak", node.kind())),
        };
        let node = arena.alloc(PageBreakNode::new(master_name));
        Ok(RenderNode::PageBreak(node))
    }

    pub fn new(master_name: Option<TextStr>) -> Self {
        Self {
            master_name,
            style: Arc::new(ComputedStyle::default()),
        }
    }
}

impl LayoutNode for PageBreakNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, _env: &mut LayoutEnvironment, _constraints: BoxConstraints) -> Size {
        Size::zero()
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        _constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if break_state.is_some() {
            return Ok(LayoutResult::Finished);
        }

        if !ctx.is_empty() || ctx.cursor_y() > 0.0 {
            Ok(LayoutResult::Break(NodeState::Atomic))
        } else {
            Ok(LayoutResult::Finished)
        }
    }

    fn check_for_page_break(&self) -> Option<Option<TextStr>> {
        Some(self.master_name.clone())
    }
}