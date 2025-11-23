use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::nodes::paragraph::ParagraphNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;

pub struct HeadingBuilder;

impl NodeBuilder for HeadingBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::Heading(HeadingNode::new(node, engine, parent_style)?))
    }
}

/// A `LayoutNode` for headings.
#[derive(Debug, Clone)]
pub struct HeadingNode {
    id: Option<String>,
    p_node: ParagraphNode,
}

impl HeadingNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        let (meta, _level, children) = match node {
            IRNode::Heading {
                meta,
                level,
                children,
            } => (meta, level, children),
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

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        self.p_node.measure(env, constraints)
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutContext,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }
        self.p_node.layout(ctx)
    }
}