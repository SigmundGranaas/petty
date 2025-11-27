use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::nodes::paragraph::ParagraphNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;
use std::any::Any;
use crate::core::idf::IRNode;

pub struct HeadingBuilder;

impl NodeBuilder for HeadingBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(Box::new(HeadingNode::new(node, engine, parent_style)?))
    }
}

#[derive(Debug)]
pub struct HeadingNode {
    id: Option<String>,
    p_node: ParagraphNode,
}

impl HeadingNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        let (meta, children) = match node {
            IRNode::Heading {
                meta,
                children,
                ..
            } => (meta, children),
            _ => return Err(LayoutError::BuilderMismatch("Heading", node.kind())),
        };

        let p_ir = IRNode::Paragraph {
            meta: meta.clone(),
            children: children.clone(),
        };

        let p_node = ParagraphNode::new(&p_ir, engine, parent_style)?;

        Ok(Self {
            id: meta.id.clone(),
            p_node,
        })
    }
}

impl LayoutNode for HeadingNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        self.p_node.style()
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        self.p_node.measure(env, constraints)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>,
    ) -> Result<LayoutResult, LayoutError> {
        self.p_node.layout(ctx, constraints, break_state)
    }
}