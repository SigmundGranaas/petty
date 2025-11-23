use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::nodes::list_item::ListItemNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;

pub struct ListBuilder;

impl NodeBuilder for ListBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::List(ListNode::new(node, engine, parent_style)?))
    }
}

#[derive(Debug, Clone)]
pub struct ListNode {
    id: Option<String>,
    block: BlockNode,
    _depth: usize,
}

impl ListNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        Self::new_with_depth(node, engine, parent_style, 0)
    }

    pub fn new_with_depth(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        depth: usize,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (meta, ir_children, start) = match node {
            IRNode::List {
                meta,
                children,
                start,
                ..
            } => (meta, children, *start),
            _ => return Err(LayoutError::BuilderMismatch("List", node.kind())),
        };

        let start_index = start.unwrap_or(1);

        let mut children = Vec::new();
        for (i, child_ir) in ir_children.iter().enumerate() {
            if let IRNode::List { .. } = child_ir {
                children.push(RenderNode::List(ListNode::new_with_depth(
                    child_ir,
                    engine,
                    style.clone(),
                    depth + 1,
                )?));
            } else if let IRNode::ListItem { .. } = child_ir {
                children.push(RenderNode::ListItem(ListItemNode::new(
                    child_ir,
                    engine,
                    style.clone(),
                    i + start_index,
                    depth,
                )?));
            } else {
                children.push(engine.build_layout_node_tree(child_ir, style.clone())?);
            }
        }

        let block = BlockNode::new_from_children(meta.id.clone(), children, style);
        Ok(Self {
            id: meta.id.clone(),
            block,
            _depth: depth,
        })
    }
}

impl LayoutNode for ListNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        self.block.style()
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        self.block.measure(env, constraints)
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
        self.block.layout(ctx)
    }
}