// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/list.rs
use crate::core::idf::IRNode;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::nodes::list_item::ListItemNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::any::Any;
use std::sync::Arc;

/// A `LayoutNode` for list containers (`<ul>`, `<ol>`).
/// Its primary role is to create `ListItemNode` children, passing them their index.
/// The actual vertical stacking logic is delegated to an inner `BlockNode`.
#[derive(Debug)]
pub struct ListNode {
    // Internally, a list is just a block with special children.
    block: BlockNode,
}

impl ListNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let ir_children = match node {
            IRNode::List { children, .. } => children,
            _ => panic!("ListNode must be created from an IRNode::List"),
        };

        let children: Vec<Box<dyn LayoutNode>> = ir_children
            .iter()
            .enumerate()
            .map(|(i, child_ir)| {
                if let IRNode::ListItem { .. } = child_ir {
                    Box::new(ListItemNode::new(child_ir, engine, style.clone(), i + 1))
                        as Box<dyn LayoutNode>
                } else {
                    // Non-ListItem in a List, lay out as a simple block.
                    log::warn!("Found non-ListItem node inside a List. This is not recommended.");
                    engine.build_layout_node_tree(child_ir, style.clone())
                }
            })
            .collect();

        let block = BlockNode::new_from_children(children, style);
        Self { block }
    }
}

impl LayoutNode for ListNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        self.block.style()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        self.block.measure(engine, available_width);
    }
    fn measure_content_height(&mut self, engine: &LayoutEngine, available_width: f32) -> f32 {
        self.block.measure_content_height(engine, available_width)
    }
    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        // Delegate directly to the inner BlockNode.
        self.block.layout(ctx)
    }
}