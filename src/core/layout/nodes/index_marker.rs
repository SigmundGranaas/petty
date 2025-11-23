use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    IndexEntry, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;

pub struct IndexMarkerBuilder;

impl NodeBuilder for IndexMarkerBuilder {
    fn build(
        &self,
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::IndexMarker(IndexMarkerNode::new(node)?))
    }
}

#[derive(Debug, Clone)]
pub struct IndexMarkerNode {
    term: String,
    style: Arc<ComputedStyle>,
}

impl IndexMarkerNode {
    pub fn new(node: &IRNode) -> Result<Self, LayoutError> {
        let term = match node {
            IRNode::IndexMarker { term, .. } => term.clone(),
            _ => return Err(LayoutError::BuilderMismatch("IndexMarker", node.kind())),
        };
        Ok(Self {
            term,
            style: Arc::new(ComputedStyle::default()),
        })
    }
}

impl LayoutNode for IndexMarkerNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, _env: &LayoutEnvironment, _constraints: BoxConstraints) -> Size {
        Size::zero()
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutContext,
    ) -> Result<LayoutResult, LayoutError> {
        let entry = IndexEntry {
            local_page_index: ctx.local_page_index,
            y_pos: ctx.cursor.1 + ctx.bounds.y,
        };
        ctx.index_entries
            .entry(self.term.clone())
            .or_default()
            .push(entry);

        Ok(LayoutResult::Full)
    }
}