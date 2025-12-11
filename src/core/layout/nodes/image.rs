// src/core/layout/nodes/image.rs

use crate::core::idf::{IRNode, InlineMetadata, TextStr};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{
    ImageElement, LayoutElement, LayoutError, PositionedElement,
};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

pub struct ImageBuilder;

impl NodeBuilder for ImageBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        ImageNode::build(node, engine, parent_style, store)
    }
}

// FIX: Added lifetime 'a to ImageNode to hold references
#[derive(Debug, Clone)]
pub struct ImageNode<'a> {
    id: Option<&'a str>,
    src: &'a str,
    style: &'a ComputedStyle,
}

impl<'a> ImageNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        // We cannot use store.bump directly inside new() easily without passing it,
        // so we inline construction or pass store.

        let IRNode::Image { meta, src } = node else {
            return Err(LayoutError::BuilderMismatch("Image", node.kind()));
        };
        let style = engine.compute_style(
            &meta.style_sets,
            meta.style_override.as_ref(),
            &parent_style,
        );

        let id = meta.id.as_ref().map(|s| store.alloc_str(s));
        let src = store.alloc_str(src);
        let style_ref = store.cache_style(style);

        let item = store.bump.alloc(Self {
            id,
            src,
            style: style_ref,
        });
        Ok(RenderNode::Image(item))
    }

    // Used for inline images in paragraphs
    pub fn new_inline(
        meta: &InlineMetadata,
        src: TextStr,
        engine: &LayoutEngine,
        parent_style: &Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style);
        let src_ref = store.alloc_str(&src);
        let style_ref = store.cache_style(style);

        Ok(Self {
            id: None,
            src: src_ref,
            style: style_ref,
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
            Some(Dimension::Percent(_)) | _ => width,
        };

        Size::new(width, height)
    }
}

impl<'a> LayoutNode for ImageNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style
    }

    fn measure(&self, _env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
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
        if break_state.is_some() {
            // resume logic...
        }

        if let Some(id) = self.id {
            ctx.register_anchor(id);
        }

        let size = self.resolve_sizes(constraints);
        let total_height = self.style.box_model.margin.top
            + size.height
            + self.style.box_model.margin.bottom;

        if total_height > ctx.bounds().height {
            return Ok(LayoutResult::Finished);
        }

        if total_height > ctx.available_height() && !ctx.is_empty() {
            return Ok(LayoutResult::Break(NodeState::Atomic));
        }

        ctx.advance_cursor(self.style.box_model.margin.top);

        let element = PositionedElement {
            x: self.style.box_model.margin.left,
            y: 0.0,
            width: size.width,
            height: size.height,
            element: LayoutElement::Image(ImageElement {
                src: self.src.to_string(), // Copy to output String
            }),
            style: Arc::new(self.style.clone()),
        };
        ctx.push_element(element);

        ctx.advance_cursor(size.height);
        ctx.advance_cursor(self.style.box_model.margin.bottom);

        Ok(LayoutResult::Finished)
    }
}