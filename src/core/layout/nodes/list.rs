// src/core/layout/nodes/list.rs

use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::nodes::list_item::ListItemNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use bumpalo::Bump;
use std::sync::Arc;

pub struct ListBuilder;

impl NodeBuilder for ListBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        ListNode::build(node, engine, parent_style, arena)
    }
}

#[derive(Debug)]
pub struct ListNode<'a> {
    // Composed of a block node
    block: BlockNode<'a>,
}

impl<'a> ListNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let node = arena.alloc(Self::new_with_depth(node, engine, parent_style, 0, arena)?);
        Ok(RenderNode::List(node))
    }

    pub fn new_with_depth(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        depth: usize,
        arena: &'a Bump,
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

        let mut children_vec = Vec::new();
        for (i, child_ir) in ir_children.iter().enumerate() {
            if let IRNode::List { .. } = child_ir {
                let child_node = Self::new_with_depth(child_ir, engine, style.clone(), depth + 1, arena)?;
                children_vec.push(RenderNode::List(arena.alloc(child_node)));
            } else if let IRNode::ListItem { .. } = child_ir {
                let child_node = ListItemNode::new(child_ir, engine, style.clone(), i + start_index, depth, arena)?;
                children_vec.push(RenderNode::ListItem(arena.alloc(child_node)));
            } else {
                children_vec.push(engine.build_layout_node_tree(child_ir, style.clone(), arena)?);
            }
        }

        let block = BlockNode::new_from_children(meta.id.clone(), children_vec, style, arena);
        Ok(Self { block })
    }
}

impl<'a> LayoutNode for ListNode<'a> {
    fn style(&self) -> &Arc<ComputedStyle> {
        self.block.style()
    }

    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
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