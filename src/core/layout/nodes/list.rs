// src/core/layout/nodes/list.rs

use crate::core::idf::IRNode;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::nodes::list_item::ListItemNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::sync::Arc;

#[derive(Debug)]
pub struct ListNode {
    block: BlockNode,
}

impl ListNode {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::List(Box::new(Self::new(
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
        Self::new_with_depth(node, engine, parent_style, 0)
    }

    pub fn new_with_depth(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        depth: usize,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);

        let IRNode::List {
            meta,
            children: ir_children,
            start,
            ..
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("List", node.kind()));
        };

        let start_index = start.unwrap_or(1);

        let mut children: Vec<RenderNode> = Vec::new();
        for (i, child_ir) in ir_children.iter().enumerate() {
            if let IRNode::List { .. } = child_ir {
                children.push(RenderNode::List(Box::new(ListNode::new_with_depth(
                    child_ir,
                    engine,
                    style.clone(),
                    depth + 1,
                )?)));
            } else if let IRNode::ListItem { .. } = child_ir {
                children.push(RenderNode::ListItem(Box::new(ListItemNode::new(
                    child_ir,
                    engine,
                    style.clone(),
                    i + start_index,
                    depth,
                )?)));
            } else {
                children.push(engine.build_layout_node_tree(child_ir, style.clone())?);
            }
        }

        let block = BlockNode::new_from_children(meta.id.clone(), children, style);
        Ok(Self { block })
    }
}

impl LayoutNode for ListNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        self.block.style()
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        self.block.measure(env, constraints)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        self.block.layout(ctx, constraints, break_state)
    }
}