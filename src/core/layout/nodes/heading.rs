// src/core/layout/nodes/heading.rs

use crate::core::idf::IRNode;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::nodes::paragraph::ParagraphNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;

#[derive(Debug)]
pub struct HeadingNode {
    id: Option<String>,
    p_node: ParagraphNode,
}

impl HeadingNode {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::Heading(Box::new(Self::new(
            node,
            engine,
            parent_style,
        )?)))
    }

    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<Self, LayoutError> {
        let IRNode::Heading {
            meta,
            children,
            ..
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("Heading", node.kind()));
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
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        self.p_node.layout(ctx, constraints, break_state)
    }
}