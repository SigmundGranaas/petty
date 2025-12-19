use crate::engine::{LayoutEngine, LayoutStore};
use crate::nodes::RenderNode;
use crate::style::ComputedStyle;
use crate::LayoutError;
use std::sync::Arc;
use petty_idf::IRNode;

/// Trait defining the interface for building a RenderNode from an IRNode.
///
/// While we utilize static dispatch in the engine for performance, this trait
/// ensures interface consistency across all node builders.
pub trait NodeBuilder: Send + Sync {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError>;
}