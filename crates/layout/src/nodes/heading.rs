// src/core/layout/nodes/heading.rs

use super::RenderNode;
use crate::LayoutError;
use crate::engine::{LayoutEngine, LayoutStore};
use crate::interface::{LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState};
use crate::nodes::paragraph::ParagraphNode;
use crate::style::ComputedStyle;
use petty_idf::{IRNode, TextStr};
use petty_types::geometry::{BoxConstraints, Size};
use std::sync::Arc;

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
        let IRNode::Heading { meta, children, .. } = node else {
            return Err(LayoutError::BuilderMismatch("Heading", node.kind()));
        };

        // Construct a temporary Paragraph IR node to delegate logic
        let p_ir = IRNode::Paragraph {
            meta: meta.clone(),
            children: children.clone(),
        };

        // Use the Paragraph builder logic (via static build)
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

    fn measure(
        &self,
        env: &LayoutEnvironment,
        constraints: BoxConstraints,
    ) -> Result<Size, LayoutError> {
        self.p_node.measure(env, constraints)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        // Register ID if present
        if let Some(id) = &self.id {
            ctx.register_anchor(id);
        }
        self.p_node.layout(ctx, constraints, break_state)
    }
}
