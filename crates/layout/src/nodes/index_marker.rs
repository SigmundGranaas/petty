// src/core/layout/nodes/index_marker.rs

use petty_idf::IRNode;
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
pub struct IndexMarkerNode<'a> {
    term: &'a str,
    style: Arc<ComputedStyle>,
}

impl<'a> IndexMarkerNode<'a> {
    pub fn build(
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let IRNode::IndexMarker { term, .. } = node else {
            return Err(LayoutError::BuilderMismatch("IndexMarker", node.kind()));
        };

        let term_ref = store.alloc_str(term);
        // Default style needed for layout node trait, even if invisible
        let style = Arc::new(ComputedStyle::default());
        let style_ref = store.cache_style(style);

        let node = store.bump.alloc(Self {
            term: term_ref,
            style: style_ref,
        });
        Ok(RenderNode::IndexMarker(node))
    }
}

impl<'a> LayoutNode for IndexMarkerNode<'a> {
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
        _break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        ctx.register_index_entry(self.term);
        Ok(LayoutResult::Finished)
    }
}