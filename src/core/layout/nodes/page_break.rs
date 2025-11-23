use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;

pub struct PageBreakBuilder;

impl NodeBuilder for PageBreakBuilder {
    fn build(
        &self,
        node: &IRNode,
        _engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        let master_name = match node {
            IRNode::PageBreak { master_name } => master_name.clone(),
            _ => return Err(LayoutError::BuilderMismatch("PageBreak", node.kind())),
        };
        Ok(RenderNode::PageBreak(PageBreakNode::new(master_name)))
    }
}

#[derive(Debug, Clone)]
pub struct PageBreakNode {
    pub master_name: Option<String>,
    style: Arc<ComputedStyle>,
}

impl PageBreakNode {
    pub fn new(master_name: Option<String>) -> Self {
        Self {
            master_name,
            style: Arc::new(ComputedStyle::default()),
        }
    }
}

impl LayoutNode for PageBreakNode {
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
        if !ctx.is_empty() || ctx.cursor.1 > 0.0 {
            Ok(LayoutResult::Partial(RenderNode::PageBreak(self.clone())))
        } else {
            Ok(LayoutResult::Full)
        }
    }
}