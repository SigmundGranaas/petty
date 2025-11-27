// src/core/layout/nodes/index_marker.rs

use crate::core::idf::IRNode;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct IndexMarkerNode {
    term: String,
    style: Arc<ComputedStyle>,
}

impl IndexMarkerNode {
    pub fn build(
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::IndexMarker(Box::new(Self::new(node)?)))
    }

    pub fn new(node: &IRNode) -> Result<Self, LayoutError> {
        let IRNode::IndexMarker { term, .. } = node else {
            return Err(LayoutError::BuilderMismatch("IndexMarker", node.kind()));
        };
        Ok(Self {
            term: term.clone(),
            style: Arc::new(ComputedStyle::default()),
        })
    }
}

impl LayoutNode for IndexMarkerNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, _env: &LayoutEnvironment, _constraints: BoxConstraints) -> Size {
        Size::zero()
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        _constraints: BoxConstraints,
        _break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        ctx.register_index_entry(&self.term);
        Ok(LayoutResult::Finished)
    }
}