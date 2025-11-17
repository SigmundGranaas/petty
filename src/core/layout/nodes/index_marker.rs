use crate::core::idf::IRNode;
use crate::core::layout::node::{IndexEntry, LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::any::Any;
use std::sync::Arc;

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

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        let entry = IndexEntry {
            local_page_index: ctx.local_page_index,
            y_pos: ctx.cursor.1 + ctx.bounds.y,
        };
        ctx.index_entries
            .borrow_mut()
            .entry(self.term.clone())
            .or_default()
            .push(entry);

        // This node consumes no space.
        Ok(LayoutResult::Full)
    }
}