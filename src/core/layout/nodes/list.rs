// src/core/layout/nodes/list.rs

use crate::core::idf::IRNode;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::base::geometry::{BoxConstraints, Size};
use crate::core::layout::interface::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState,
};
use super::RenderNode;
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::nodes::list_item::ListItemNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::sync::Arc;

#[derive(Debug)]
pub struct ListNode<'a> {
    // List is essentially a wrapper around a BlockNode that manages numbering depth
    block: BlockNode<'a>,
}

impl<'a> ListNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let node = store.bump.alloc(Self::new_with_depth(node, engine, parent_style, 0, store)?);
        Ok(RenderNode::List(node))
    }

    pub fn new_with_depth(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        depth: usize,
        store: &'a LayoutStore,
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
                // Recursive list
                let child_node = Self::new_with_depth(child_ir, engine, style.clone(), depth + 1, store)?;
                children_vec.push(RenderNode::List(store.bump.alloc(child_node)));
            } else if let IRNode::ListItem { .. } = child_ir {
                // List item with context
                let child_node = ListItemNode::new(child_ir, engine, style.clone(), i + start_index, depth, store)?;
                children_vec.push(RenderNode::ListItem(store.bump.alloc(child_node)));
            } else {
                // Other nodes in list container
                children_vec.push(engine.build_layout_node_tree(child_ir, style.clone(), store)?);
            }
        }

        // Delegate layout logic to BlockNode
        let block = BlockNode::new_from_children(meta.id.clone(), children_vec, style, store);
        Ok(Self { block })
    }
}

impl<'a> LayoutNode for ListNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.block.style()
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Result<Size, LayoutError> {
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