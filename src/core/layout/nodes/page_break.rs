// src/core/layout/nodes/page_break.rs

use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::sync::Arc;

pub struct PageBreakBuilder;

impl NodeBuilder for PageBreakBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        PageBreakNode::build(node, engine, parent_style, store)
    }
}

// FIX: Added lifetime 'a
#[derive(Debug, Clone)]
pub struct PageBreakNode<'a> {
    pub master_name: Option<&'a str>,
    style: &'a ComputedStyle,
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

        // FIX: Allocate master_name in arena
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
        self.style
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
        // Convert &'a str to String for interface compatibility.
        // This is safe because check_for_page_break returns a new String (TextStr) in the Option.
        Some(self.master_name.map(|s| s.to_string()))
    }
}