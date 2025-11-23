use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    IndexEntry, LayoutBuffer, LayoutEnvironment, LayoutNode, LayoutResult,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::any::Any;
use std::sync::Arc;

pub struct IndexMarkerBuilder;

impl NodeBuilder for IndexMarkerBuilder {
    fn build(
        &self,
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
    ) -> Box<dyn LayoutNode> {
        Box::new(IndexMarkerNode::new(node))
    }
}

/// A special `LayoutNode` that represents an index term marker.
/// It is invisible and its only purpose is to record its position during layout.
#[derive(Debug, Clone)]
pub struct IndexMarkerNode {
    term: String,
    style: Arc<ComputedStyle>,
}

impl IndexMarkerNode {
    pub fn new(node: &IRNode) -> Self {
        let term = match node {
            IRNode::IndexMarker { term, .. } => term.clone(),
            _ => panic!("IndexMarkerNode must be created from IRNode::IndexMarker"),
        };
        Self {
            term,
            style: Arc::new(ComputedStyle::default()),
        }
    }
}

impl LayoutNode for IndexMarkerNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, _env: &LayoutEnvironment, _constraints: BoxConstraints) -> Size {
        Size::zero()
    }

    fn layout(
        &mut self,
        env: &LayoutEnvironment,
        buf: &mut LayoutBuffer,
    ) -> Result<LayoutResult, LayoutError> {
        let entry = IndexEntry {
            local_page_index: env.local_page_index,
            y_pos: buf.cursor.1 + buf.bounds.y,
        };
        buf.index_entries
            .entry(self.term.clone())
            .or_default()
            .push(entry);

        // This node consumes no space.
        Ok(LayoutResult::Full)
    }
}