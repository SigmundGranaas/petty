use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{
    ImageElement, LayoutElement, LayoutEngine, LayoutError, PositionedElement,
};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;
use std::any::Any;
use crate::core::idf::{IRNode, InlineMetadata};

pub struct ImageBuilder;

impl NodeBuilder for ImageBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(Box::new(ImageNode::new(node, engine, parent_style)?))
    }
}

#[derive(Debug, Clone)]
pub struct ImageNode {
    id: Option<String>,
    src: String,
    style: Arc<ComputedStyle>,
    // Removed cached width/height, recompute in measure
}

impl ImageNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        let (meta, src) = match node {
            IRNode::Image { meta, src } => (meta, src.clone()),
            _ => return Err(LayoutError::BuilderMismatch("Image", node.kind())),
        };
        let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), &parent_style);

        Ok(Self {
            id: meta.id.clone(),
            src,
            style,
        })
    }

    pub fn new_inline(meta: &InlineMetadata, src: String, engine: &LayoutEngine, parent_style: &Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style);
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
            Some(Dimension::Percent(_)) | _ => width, // Aspect ratio preservation placeholder
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
        let total_height = self.style.box_model.margin.top + content_size.height + self.style.box_model.margin.bottom;
        Size::new(content_size.width, total_height)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>,
    ) -> Result<LayoutResult, LayoutError> {
        if break_state.is_some() {
            // Image was already rendered or skipped?
            // Simplification: Image is atomic. If it's in break state, we assume it didn't fit previously and now we render it.
        }

        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }

        let size = self.resolve_sizes(constraints);
        let total_height = self.style.box_model.margin.top + size.height + self.style.box_model.margin.bottom;

        if total_height > ctx.bounds.height {
            // Exceeds page height. Skip it to avoid infinite loops and match test expectation.
            return Ok(LayoutResult::Finished);
        }

        if total_height > ctx.available_height() && !ctx.is_empty() {
            // Push to next page
            return Ok(LayoutResult::Break(Box::new(())));
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