use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{AnchorLocation, LayoutBuffer, LayoutEnvironment, LayoutNode, LayoutResult};
use crate::core::layout::nodes::paragraph::ParagraphNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use std::any::Any;
use std::sync::Arc;
use crate::core::idf::IRNode;

/// A `LayoutNode` for headings (`<h1>`, `<h2>`, etc.).
/// It behaves like a paragraph but also registers itself as an anchor.
#[derive(Debug, Clone)]
pub struct HeadingNode {
    id: Option<String>,
    // Delegate paragraph-like behavior to an actual ParagraphNode.
    p_node: ParagraphNode,
}

impl HeadingNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let (meta, _level, children) = match node {
            IRNode::Heading { meta, level, children } => (meta, level, children),
            _ => panic!("HeadingNode must be created from an IRNode::Heading"),
        };

        // Create a synthetic Paragraph IRNode to reuse its logic.
        let p_ir = IRNode::Paragraph {
            meta: meta.clone(),
            children: children.clone(),
        };

        // TODO: In a future step, we might apply specific default styles based on `level`.
        // For now, it just inherits like a normal paragraph.
        let p_node = ParagraphNode::new(&p_ir, engine, parent_style);

        Self {
            id: meta.id.clone(),
            p_node,
        }
    }
}

impl LayoutNode for HeadingNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        self.p_node.style()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        self.p_node.measure(env, constraints)
    }

    fn layout(&mut self, env: &LayoutEnvironment, buf: &mut LayoutBuffer) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            // The anchor position is right before the top margin is applied.
            let location = AnchorLocation {
                local_page_index: env.local_page_index,
                y_pos: buf.cursor.1 + buf.bounds.y,
            };
            buf.defined_anchors.insert(id.clone(), location);
        }

        // Delegate the actual layout work to the inner ParagraphNode.
        self.p_node.layout(env, buf)
    }
}