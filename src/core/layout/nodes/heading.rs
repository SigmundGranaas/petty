// src/core/layout/nodes/heading.rs

use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::nodes::paragraph::ParagraphNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::sync::Arc;

pub struct HeadingBuilder;

impl NodeBuilder for HeadingBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        HeadingNode::build(node, engine, parent_style, store)
    }
}

#[derive(Debug)]
pub struct HeadingNode<'a> {
    id: Option<TextStr>,
    p_node: &'a ParagraphNode<'a>,
}

impl<'a> HeadingNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
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

        let p_render_node = ParagraphNode::build(&p_ir, engine, parent_style, store)?;

        let p_node_ref = match p_render_node {
            RenderNode::Paragraph(p) => p,
            _ => panic!("Paragraph build failed"),
        };

        let node = store.bump.alloc(Self {
            id: meta.id.clone(),
            p_node: p_node_ref,
        });

        Ok(RenderNode::Heading(node))
    }
}

impl<'a> LayoutNode for HeadingNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.p_node.style()
    }

    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
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