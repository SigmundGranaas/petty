use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{AnchorLocation, LayoutBuffer, LayoutEnvironment, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{ImageElement, LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;
use crate::core::idf::IRNode;

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

    fn resolve_sizes(&mut self, constraints: BoxConstraints) -> Size {
        let available_width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            f32::INFINITY
        };

        self.width = match &self.style.width {
            Some(Dimension::Pt(w)) => *w,
            Some(Dimension::Percent(p)) => if available_width.is_finite() { available_width * (p / 100.0) } else { 0.0 },
            _ => if available_width.is_finite() { available_width } else { 100.0 }, // Fallback size for infinite
        };
        self.height = match &self.style.height {
            Some(Dimension::Pt(h)) => *h,
            // A percentage height for a block image usually resolves against the container height,
            // which we don't know here. We'll treat it as auto.
            Some(Dimension::Percent(_)) | _ => self.width, // Assume square aspect ratio for auto
        };

        Size::new(self.width, self.height)
    }
}

impl LayoutNode for ImageNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, _env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let content_size = self.resolve_sizes(constraints);
        let total_height = self.style.margin.top + content_size.height + self.style.margin.bottom;
        Size::new(content_size.width, total_height)
    }

    fn layout(&mut self, env: &LayoutEnvironment, buf: &mut LayoutBuffer) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: env.local_page_index,
                y_pos: buf.cursor.1 + buf.bounds.y,
            };
            buf.defined_anchors.insert(id.clone(), location);
        }

        let total_height = self.style.margin.top + self.height + self.style.margin.bottom;

        if total_height > buf.bounds.height {
            return Err(LayoutError::ElementTooLarge(total_height, buf.bounds.height));
        }

        if total_height > buf.available_height() && (!buf.is_empty() || buf.cursor.1 > 0.0) {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }

        buf.advance_cursor(self.style.margin.top);

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
        buf.push_element(element);

        buf.advance_cursor(self.height);
        buf.advance_cursor(self.style.margin.bottom);

        Ok(LayoutResult::Full)
    }
}