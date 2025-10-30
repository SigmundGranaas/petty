use crate::core::idf::IRNode;
use crate::core::layout::node::{AnchorLocation, LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{ImageElement, LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ImageNode {
    id: Option<String>,
    src: String,
    style: Arc<ComputedStyle>,
    width: f32,
    height: f32,
}

impl ImageNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let (meta, src) = match node {
            IRNode::Image { meta, src } => (meta, src.clone()),
            _ => panic!("ImageNode must be created from IRNode::Image"),
        };
        let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), &parent_style);

        Self {
            id: meta.id.clone(),
            src,
            style,
            width: 0.0, // Resolved in measure pass
            height: 0.0,
        }
    }

    fn resolve_sizes(&mut self, available_width: f32) {
        self.width = match &self.style.width {
            Some(Dimension::Pt(w)) => *w,
            Some(Dimension::Percent(p)) => available_width * (p / 100.0),
            _ => available_width,
        };
        self.height = match &self.style.height {
            Some(Dimension::Pt(h)) => *h,
            // A percentage height for a block image usually resolves against the container height,
            // which we don't know here. We'll treat it as auto.
            Some(Dimension::Percent(_)) | _ => self.width, // Assume square aspect ratio for auto
        };
    }
}

impl LayoutNode for ImageNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, _engine: &LayoutEngine, available_width: f32) {
        self.resolve_sizes(available_width);
    }

    fn measure_content_height(&mut self, _engine: &LayoutEngine, available_width: f32) -> f32 {
        self.resolve_sizes(available_width);
        self.style.margin.top + self.height + self.style.margin.bottom
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.borrow_mut().insert(id.clone(), location);
        }

        let total_height = self.style.margin.top + self.height + self.style.margin.bottom;

        if total_height > ctx.bounds.height {
            return Err(LayoutError::ElementTooLarge(total_height, ctx.bounds.height));
        }

        if total_height > ctx.available_height() && (!ctx.is_empty() || ctx.cursor.1 > 0.0) {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }

        ctx.advance_cursor(self.style.margin.top);

        let element = PositionedElement {
            x: self.style.margin.left,
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
        ctx.advance_cursor(self.style.margin.bottom);

        Ok(LayoutResult::Full)
    }
}