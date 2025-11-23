use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutBuffer, LayoutEnvironment, LayoutNode, LayoutResult,
};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::nodes::list_item::ListItemNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::any::Any;
use std::sync::Arc;

pub struct ListBuilder;

impl NodeBuilder for ListBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Box<dyn LayoutNode> {
        Box::new(ListNode::new(node, engine, parent_style))
    }
}

/// A `LayoutNode` for list containers (`<ul>`, `<ol>`).
/// Its primary role is to create `ListItemNode` children, passing them their index.
/// The actual vertical stacking logic is delegated to an inner `BlockNode`.
#[derive(Debug, Clone)]
pub struct ListNode {
    id: Option<String>,
    // Internally, a list is just a block with special children.
    block: BlockNode,
    _depth: usize,
}

impl ListNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        Self::new_with_depth(node, engine, parent_style, 0)
    }

    pub fn new_with_depth(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        depth: usize,
    ) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (meta, ir_children, start) = match node {
            IRNode::List {
                meta,
                children,
                start,
                ..
            } => (meta, children, *start),
            _ => panic!("ListNode must be created from an IRNode::List"),
        };

        let start_index = start.unwrap_or(1);

        let children: Vec<Box<dyn LayoutNode>> = ir_children
            .iter()
            .enumerate()
            .map(|(i, child_ir)| {
                // If a child is another list, increase the depth.
                if let IRNode::List { .. } = child_ir {
                    return Box::new(ListNode::new_with_depth(
                        child_ir,
                        engine,
                        style.clone(),
                        depth + 1,
                    )) as Box<dyn LayoutNode>;
                }

                if let IRNode::ListItem { .. } = child_ir {
                    Box::new(ListItemNode::new(
                        child_ir,
                        engine,
                        style.clone(),
                        i + start_index, // Use correct start index
                        depth,
                    )) as Box<dyn LayoutNode>
                } else {
                    // Non-ListItem in a List, lay out as a simple block.
                    log::warn!("Found non-ListItem node inside a List. This is not recommended.");
                    engine.build_layout_node_tree(child_ir, style.clone())
                }
            })
            .collect();

        let block = BlockNode::new_from_children(meta.id.clone(), children, style);
        Self {
            id: meta.id.clone(),
            block,
            _depth: depth,
        }
    }
}

impl LayoutNode for ListNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        self.block.style()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        self.block.measure(env, constraints)
    }

    fn layout(
        &mut self,
        env: &LayoutEnvironment,
        buf: &mut LayoutBuffer,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: env.local_page_index,
                y_pos: buf.cursor.1 + buf.bounds.y,
            };
            buf.defined_anchors.insert(id.clone(), location);
        }
        // Delegate directly to the inner BlockNode.
        self.block.layout(env, buf)
    }
}