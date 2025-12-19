use crate::engine::{LayoutEngine, LayoutStore};
use petty_types::geometry::{self, BoxConstraints, Size};
use crate::interface::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState,
};
use super::RenderNode;
use crate::style::ComputedStyle;
use crate::{LayoutElement, LayoutError, PositionedElement, ImageElement};
use petty_style::dimension::Dimension;
use crate::painting::box_painter::create_background_and_borders;
use std::sync::Arc;
use petty_idf::IRNode;

#[derive(Debug, Clone)]
pub struct ImageNode<'a> {
    id: Option<&'a str>,
    src: &'a str,
    style: Arc<ComputedStyle>,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> ImageNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let node = store.bump.alloc(Self::new(node, engine, parent_style, store)?);
        Ok(RenderNode::Image(node))
    }

    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<Self, LayoutError> {
        let (id_str, src_str, meta) = match node {
            IRNode::Image { meta, src } => (&meta.id, src, meta),
            _ => return Err(LayoutError::BuilderMismatch("Image", node.kind())),
        };

        let style = engine.compute_style(
            &meta.style_sets,
            meta.style_override.as_ref(),
            &parent_style,
        );

        let style_ref = store.cache_style(style);
        // Alloc string content into the bump arena for lifetime 'a
        let src_ref = store.alloc_str(src_str);
        let id_ref = id_str.as_ref().map(|s| store.alloc_str(s));

        Ok(Self {
            id: id_ref,
            src: src_ref,
            style: style_ref,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn new_inline(
        meta: &petty_idf::InlineMetadata,
        src: String,
        engine: &LayoutEngine,
        parent_style: &Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(
            &meta.style_sets,
            meta.style_override.as_ref(),
            parent_style,
        );
        let style_ref = store.cache_style(style);
        let src_ref = store.alloc_str(&src);

        Ok(Self {
            id: None,
            src: src_ref,
            style: style_ref,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<'a> LayoutNode for ImageNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style.as_ref()
    }

    fn measure(&self, _env: &LayoutEnvironment, constraints: BoxConstraints) -> Result<Size, LayoutError> {
        let w = match self.style.box_model.width {
            Some(Dimension::Pt(v)) => v,
            _ => 100.0,
        };

        let h = match self.style.box_model.height {
            Some(Dimension::Pt(v)) => v,
            _ => 100.0,
        };

        let width = constraints.constrain_width(w + self.style.padding_x() + self.style.border_x());
        let height = constraints.constrain_height(h + self.style.padding_y() + self.style.border_y());

        Ok(Size::new(width, height))
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        _break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = self.id {
            ctx.register_anchor(id);
        }

        // If we have a break state (Atomic), it means we pushed to the next page.
        // We should just continue layout as normal (from scratch) on this new page.
        // We don't return Finished immediately.

        let size = self.measure(&ctx.env, constraints)?;

        // Safety check: if image is taller than the page, skip it to avoid infinite loops
        if size.height > ctx.bounds().height {
            return Ok(LayoutResult::Finished);
        }

        if ctx.prepare_for_block(self.style.box_model.margin.top) {
            return Ok(LayoutResult::Break(NodeState::Atomic));
        }

        if size.height > ctx.available_height() && !ctx.is_empty() {
            return Ok(LayoutResult::Break(NodeState::Atomic));
        }

        let start_y = ctx.cursor_y();

        let bg_elements = create_background_and_borders(
            ctx.bounds(),
            &self.style,
            start_y,
            size.height,
            true, true
        );
        for el in bg_elements {
            ctx.push_element_at(el, 0.0, 0.0);
        }

        let content_rect = geometry::Rect {
            x: self.style.border_left_width() + self.style.box_model.padding.left,
            y: start_y + self.style.border_top_width() + self.style.box_model.padding.top,
            width: size.width - self.style.padding_x() - self.style.border_x(),
            height: size.height - self.style.padding_y() - self.style.border_y(),
        };

        let image_el = PositionedElement {
            element: LayoutElement::Image(ImageElement { src: self.src.to_string() }),
            style: self.style.clone(),
            ..PositionedElement::from_rect(content_rect)
        };

        ctx.push_element_at(image_el, 0.0, 0.0);

        ctx.set_cursor_y(start_y + size.height);
        ctx.finish_block(self.style.box_model.margin.bottom);

        Ok(LayoutResult::Finished)
    }
}