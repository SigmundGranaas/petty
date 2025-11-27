use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;
use std::any::Any;
use crate::core::idf::IRNode;

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
        Ok(Box::new(PageBreakNode::new(master_name)))
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

    fn measure(&self, _env: &LayoutEnvironment, _constraints: BoxConstraints) -> Size {
        Size::zero()
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        _constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>,
    ) -> Result<LayoutResult, LayoutError> {
        if break_state.is_some() {
            return Ok(LayoutResult::Finished);
        }

        if !ctx.is_empty() || ctx.cursor.1 > 0.0 {
            Ok(LayoutResult::Break(Box::new(())))
        } else {
            Ok(LayoutResult::Finished)
        }
    }

    fn check_for_page_break(&self) -> Option<Option<String>> {
        Some(self.master_name.clone())
    }
}