// src/core/layout/nodes/index_marker.rs

use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::sync::Arc;

pub struct IndexMarkerBuilder;

impl NodeBuilder for IndexMarkerBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        IndexMarkerNode::build(node, engine, parent_style, store)
    }
}

// FIX: Added lifetime 'a
#[derive(Debug, Clone)]
pub struct IndexMarkerNode<'a> {
    term: &'a str,
    style: &'a ComputedStyle,
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
        // FIX: Allocate term in arena
        let term_ref = store.alloc_str(term);
        // Default style needed for layout node, even if invisible
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
        self.style
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
        ctx.register_index_entry(self.term);
        Ok(LayoutResult::Finished)
    }
}