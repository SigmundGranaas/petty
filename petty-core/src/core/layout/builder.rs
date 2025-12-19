use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::nodes::RenderNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::sync::Arc;
use crate::core::idf::IRNode;

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