use crate::core::idf::IRNode;
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

pub struct ImageBuilder;

impl NodeBuilder for ImageBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::Image(ImageNode::new(node, engine, parent_style)?))
    }
}

#[derive(Debug, Clone)]
pub struct ImageNode {
    id: Option<String>,
    src: String,
    style: Arc<ComputedStyle>,
    width: f32,
    height: f32,
}

impl ImageNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        let (meta, src) = match node {
            IRNode::Image { meta, src } => (meta, src.clone()),
            _ => return Err(LayoutError::BuilderMismatch("Image", node.kind())),
        };
        let style =
            engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), &parent_style);

        Ok(Self {
            id: meta.id.clone(),
            src,
            style,
            width: 0.0,
            height: 0.0,
        })
    }

    fn resolve_sizes(&mut self, constraints: BoxConstraints) -> Size {
        let available_width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            f32::INFINITY
        };

        self.width = match &self.style.box_model.width {
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
        self.height = match &self.style.box_model.height {
            Some(Dimension::Pt(h)) => *h,
            Some(Dimension::Percent(_)) | _ => self.width,
        };

        Size::new(self.width, self.height)
    }
}

impl LayoutNode for ImageNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, _env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let content_size = self.resolve_sizes(constraints);
        let total_height = self.style.box_model.margin.top + content_size.height + self.style.box_model.margin.bottom;
        Size::new(content_size.width, total_height)
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

        let total_height = self.style.box_model.margin.top + self.height + self.style.box_model.margin.bottom;

        if total_height > ctx.bounds.height {
            log::warn!(
                "Image node {:?} (height {:.2}) exceeds total page content height {:.2}. Skipping.",
                self.id,
                total_height,
                ctx.bounds.height
            );
            return Ok(LayoutResult::Full);
        }

        if total_height > ctx.available_height() && (!ctx.is_empty() || ctx.cursor.1 > 0.0) {
            return Ok(LayoutResult::Partial(RenderNode::Image(self.clone())));
        }

        ctx.advance_cursor(self.style.box_model.margin.top);

        let element = PositionedElement {
            x: self.style.box_model.margin.left,
            y: 0.0,
            width: self.width,
            height: self.height,
            element: LayoutElement::Image(ImageElement {
                src: self.src.clone(),
            }),
            style: self.style.clone(),
        };
        ctx.push_element(element);

        ctx.advance_cursor(self.height);
        ctx.advance_cursor(self.style.box_model.margin.bottom);

        Ok(LayoutResult::Full)
    }
}