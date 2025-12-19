// src/core/layout/nodes/page_break.rs

use petty_idf::{IRNode, TextStr};
use crate::engine::{LayoutEngine, LayoutStore};
use petty_types::geometry::{BoxConstraints, Size};
use crate::interface::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState,
};
use super::RenderNode;
use crate::style::ComputedStyle;
use crate::LayoutError;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PageBreakNode<'a> {
    pub master_name: Option<&'a str>,
    style: Arc<ComputedStyle>,
}

impl<'a> PageBreakNode<'a> {
    pub fn build(
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let master_name = match node {
            IRNode::PageBreak { master_name } => master_name,
            _ => return Err(LayoutError::BuilderMismatch("PageBreak", node.kind())),
        };

        let master_ref = master_name.as_ref().map(|s| store.alloc_str(s));
        // Needs dummy style
        let style = Arc::new(ComputedStyle::default());
        let style_ref = store.cache_style(style);

        let node = store.bump.alloc(Self {
            master_name: master_ref,
            style: style_ref,
        });
        Ok(RenderNode::PageBreak(node))
    }
}

impl<'a> LayoutNode for PageBreakNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style.as_ref()
    }

    fn measure(&self, _env: &LayoutEnvironment, _constraints: BoxConstraints) -> Result<Size, LayoutError> {
        Ok(Size::zero())
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        _constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if break_state.is_some() {
            // We've already broken, so we are finished
            return Ok(LayoutResult::Finished);
        }

        // Force a break unless we are at the very top of an empty page (unlikely for a manual break)
        // If we just return Break, the engine will create a new page.
        if !ctx.is_empty() || ctx.cursor_y() > 0.0 {
            Ok(LayoutResult::Break(NodeState::Atomic))
        } else {
            // Already at top of page, consume the break
            Ok(LayoutResult::Finished)
        }
    }

    fn check_for_page_break(&self) -> Option<Option<TextStr>> {
        // Convert &'a str to String for interface compatibility.
        Some(self.master_name.map(|s| s.to_string()))
    }
}