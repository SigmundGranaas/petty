// src/core/layout/nodes/image.rs

use crate::core::idf::{IRNode, InlineMetadata};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{
    ImageElement, LayoutElement, LayoutEngine, LayoutError, PositionedElement,
};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ImageNode {
    id: Option<String>,
    src: String,
    style: Arc<ComputedStyle>,
}

impl ImageNode {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::Image(Box::new(Self::new(
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
        let IRNode::Image { meta, src } = node else {
            return Err(LayoutError::BuilderMismatch("Image", node.kind()));
        };
        let style = engine.compute_style(
            &meta.style_sets,
            meta.style_override.as_ref(),
            &parent_style,
        );

        Ok(Self {
            id: meta.id.clone(),
            src: src.clone(),
            style,
        })
    }

    pub fn new_inline(
        meta: &InlineMetadata,
        src: String,
        engine: &LayoutEngine,
        parent_style: &Arc<ComputedStyle>,
    ) -> Result<Self, LayoutError> {
        let style =
            engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style);
        Ok(Self {
            id: None,
            src,
            style,
        })
    }

    fn resolve_sizes(&self, constraints: BoxConstraints) -> Size {
        let available_width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            f32::INFINITY
        };

        let width = match &self.style.box_model.width {
            Some(Dimension::Pt(w)) => *w,
            Some(Dimension::Percent(p)) => {
                if available_width.is_finite() {
                    available_width * (p / 100.0)
                } else {
                    0.0
                }
            }
            _ => {
                if available_width.is_finite() {
                    available_width
                } else {
                    100.0
                }
            }
        };
        let height = match &self.style.box_model.height {
            Some(Dimension::Pt(h)) => *h,
            Some(Dimension::Percent(_)) | _ => width, // Simple aspect ratio assumption if mostly square/undefined
        };

        Size::new(width, height)
    }
}

impl LayoutNode for ImageNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, _env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let content_size = self.resolve_sizes(constraints);
        let total_height = self.style.box_model.margin.top
            + content_size.height
            + self.style.box_model.margin.bottom;
        Size::new(content_size.width, total_height)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        // If we have a break state for an atomic image, it usually means we pushed it to the next page previously.
        // We just proceed to render it now.
        if break_state.is_some() {
            // No specific state resumption logic for atomic image needed
        }

        if let Some(id) = &self.id {
            ctx.register_anchor(id);
        }

        let size = self.resolve_sizes(constraints);
        let total_height = self.style.box_model.margin.top
            + size.height
            + self.style.box_model.margin.bottom;

        if total_height > ctx.bounds().height {
            // Image is taller than the entire page.
            // In a real engine we might clip or scale, here we skip/finish to avoid infinite loops.
            return Ok(LayoutResult::Finished);
        }

        if total_height > ctx.available_height() && !ctx.is_empty() {
            // Not enough space left, push to next page
            return Ok(LayoutResult::Break(NodeState::Atomic));
        }

        ctx.advance_cursor(self.style.box_model.margin.top);

        let element = PositionedElement {
            x: self.style.box_model.margin.left,
            y: 0.0,
            width: size.width,
            height: size.height,
            element: LayoutElement::Image(ImageElement {
                src: self.src.clone(),
            }),
            style: self.style.clone(),
        };
        ctx.push_element(element);

        ctx.advance_cursor(size.height);
        ctx.advance_cursor(self.style.box_model.margin.bottom);

        Ok(LayoutResult::Finished)
    }
}