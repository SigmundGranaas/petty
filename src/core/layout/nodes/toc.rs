use crate::core::idf::IRNode;
use crate::core::layout::node::{AnchorLocation, LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;

/// A `LayoutNode` that acts as a placeholder for the Table of Contents.
/// It reserves space in the layout, which will be filled in by the renderer
/// in the finalization step.
#[derive(Debug, Clone)]
pub struct TableOfContentsNode {
    id: Option<String>,
    style: Arc<ComputedStyle>,
}

impl TableOfContentsNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let meta = match node {
            IRNode::TableOfContents { meta } => meta,
            _ => panic!("TableOfContentsNode must be created from an IRNode::TableOfContents"),
        };
        let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), &parent_style);
        Self {
            id: meta.id.clone(),
            style,
        }
    }
}

impl LayoutNode for TableOfContentsNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.borrow_mut().insert(id.clone(), location);
        }

        // For now, assume the TOC fits on one page and fills the available vertical space.
        // A more advanced implementation might calculate an estimated height.
        let height = self
            .style
            .height
            .as_ref()
            .and_then(|d| match d {
                Dimension::Pt(h) => Some(*h),
                _ => None,
            })
            .unwrap_or_else(|| ctx.available_height());

        if height > ctx.available_height() && !ctx.is_empty() {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }

        let placeholder = PositionedElement {
            x: 0.0,
            y: 0.0,
            width: ctx.bounds.width,
            height,
            element: LayoutElement::TableOfContentsPlaceholder,
            style: self.style.clone(),
        };

        ctx.push_element(placeholder);
        ctx.advance_cursor(height);

        Ok(LayoutResult::Full)
    }
}