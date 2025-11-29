// src/core/layout/nodes/index_marker.rs

use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use bumpalo::Bump;
use std::sync::Arc;

pub struct IndexMarkerBuilder;

impl NodeBuilder for IndexMarkerBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        IndexMarkerNode::build(node, engine, parent_style, arena)
    }
}

#[derive(Debug, Clone)]
pub struct IndexMarkerNode {
    term: TextStr,
    style: Arc<ComputedStyle>,
}

impl IndexMarkerNode {
    pub fn build<'a>(
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let IRNode::IndexMarker { term, .. } = node else {
            return Err(LayoutError::BuilderMismatch("IndexMarker", node.kind()));
        };
        let node = arena.alloc(Self {
            term: term.clone(),
            style: Arc::new(ComputedStyle::default()),
        });
        Ok(RenderNode::IndexMarker(node))
    }
}

impl LayoutNode for IndexMarkerNode {
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
        _break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        ctx.register_index_entry(&self.term);
        Ok(LayoutResult::Finished)
    }
}