use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::elements::RectElement;
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::util::VerticalStacker;
use crate::core::layout::{LayoutElement, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::border::Border;
use crate::core::style::dimension::Dimension;
use std::sync::Arc;
use crate::core::idf::IRNode;

/// A builder for `BlockNode`s.
pub struct BlockBuilder;

impl NodeBuilder for BlockBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (id, ir_children) = match node {
            IRNode::Block { meta, children } => (meta.id.clone(), children),
            IRNode::ListItem { meta, children } => (meta.id.clone(), children),
            _ => return Err(LayoutError::BuilderMismatch("Block", node.kind())),
        };
        let children = engine.build_layout_node_children(ir_children, style.clone())?;
        Ok(RenderNode::Block(BlockNode::new_from_children(id, children, style)))
    }
}

/// A builder specifically for the Root node, which resets style inheritance.
pub struct RootBuilder;

impl NodeBuilder for RootBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        _parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        let style = engine.get_default_style();
        let children = match node {
            IRNode::Root(c) => c,
            _ => return Err(LayoutError::BuilderMismatch("Root", node.kind())),
        };
        let children_nodes = engine.build_layout_node_children(children, style.clone())?;
        Ok(RenderNode::Block(BlockNode::new_from_children(None, children_nodes, style)))
    }
}

/// A `LayoutNode` for block-level containers like `<div>`.
#[derive(Debug, Clone)]
pub struct BlockNode {
    id: Option<String>,
    children: Vec<RenderNode>,
    style: Arc<ComputedStyle>,
}

impl BlockNode {
    // Internal constructor
    pub fn new_from_children(
        id: Option<String>,
        children: Vec<RenderNode>,
        style: Arc<ComputedStyle>,
    ) -> Self {
        Self {
            id,
            children,
            style,
        }
    }
}

impl LayoutNode for BlockNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        // 1. Determine horizontal space availability
        // Use abstraction for arithmetic
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let child_constraints = self.style.content_constraints(constraints);

        // 2. Measure children
        let mut max_child_width: f32 = 0.0;
        let mut total_content_height: f32 = 0.0;

        for child in &mut self.children {
            let child_size = child.measure(env, child_constraints);
            max_child_width = max_child_width.max(child_size.width);
            total_content_height += child_size.height;
        }

        // 3. Calculate own dimensions
        let padding_y = self.style.padding_y();
        let border_y = self.style.border_y();
        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        let height = if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            margin_y + h
        } else {
            margin_y + border_y + padding_y + total_content_height
        };

        let computed_width = if constraints.has_bounded_width() {
            constraints.max_width
        } else if let Some(Dimension::Pt(w)) = self.style.box_model.width {
            w + h_deduction
        } else {
            max_child_width + h_deduction
        };

        Size::new(computed_width, height)
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

        // --- Vertical Margin Collapsing ---
        let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);

        if ctx.cursor.1 > 0.0 && margin_to_add > ctx.available_height() {
            return Ok(LayoutResult::Partial(RenderNode::Block(self.clone())));
        }
        ctx.advance_cursor(margin_to_add);
        ctx.last_v_margin = 0.0;

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();
        let border_left = self.style.border_left_width();
        let _border_right = self.style.border_right_width();

        let block_start_y_in_ctx = ctx.cursor.1;
        ctx.advance_cursor(border_top + self.style.box_model.padding.top);
        let content_start_y_in_ctx = ctx.cursor.1;

        // Capture the index where content starts so we can insert background *before* it later.
        let content_start_index = ctx.elements.len();

        let (content_height, child_last_v_margin, partial_children) = {
            let child_bounds = geom::Rect {
                x: ctx.bounds.x + border_left + self.style.box_model.padding.left,
                y: ctx.bounds.y + content_start_y_in_ctx,
                width: ctx.bounds.width
                    - self.style.padding_x()
                    - self.style.border_x(),
                height: ctx.available_height(),
            };

            ctx.with_child_bounds(child_bounds, |child_ctx| {
                // Use the abstracted pagination logic
                let split_result = VerticalStacker::layout_children(child_ctx, &mut self.children)?;
                Ok((child_ctx.cursor.1, child_ctx.last_v_margin, split_result))
            })?
        };

        // If children split, return partial.
        if let Some(remaining_children) = partial_children {
            let bg_elements = create_background_and_borders(
                ctx.bounds,
                &self.style,
                block_start_y_in_ctx,
                content_height,
            );

            for el in bg_elements.into_iter().rev() {
                ctx.elements.insert(content_start_index, el);
            }

            ctx.cursor.1 = content_start_y_in_ctx
                + content_height
                + self.style.box_model.padding.bottom
                + border_bottom;
            ctx.last_v_margin = child_last_v_margin;

            // Reset styles for next page
            let mut next_style = (*self.style).clone();
            next_style.box_model.margin.top = 0.0;
            next_style.border.top = None;
            next_style.box_model.padding.top = 0.0;
            if next_style.box_model.height.is_some() {
                next_style.box_model.height = None;
            }

            let mut next_page_block = BlockNode {
                id: self.id.clone(),
                children: remaining_children,
                style: Arc::new(next_style),
            };
            // Measure partial block just in case it's needed for layout
            next_page_block.measure(&LayoutEnvironment{ engine: ctx.engine, local_page_index: ctx.local_page_index }, BoxConstraints::tight_width(ctx.bounds.width));

            return Ok(LayoutResult::Partial(RenderNode::Block(next_page_block)));
        }

        // Full fit logic
        let fixed_height_opt = if let Some(Dimension::Pt(h)) = self.style.box_model.height { Some(h) } else { None };
        let vertical_spacing = border_top + self.style.padding_y() + border_bottom;

        let desired_border_box_height = if let Some(h) = fixed_height_opt {
            content_height.max((h - vertical_spacing).max(0.0)) + vertical_spacing
        } else {
            content_height + vertical_spacing
        };

        let available = ctx.available_height();

        if desired_border_box_height > available + 0.1 {
            // Split due to fixed height overflow
            let taken_height = available;

            let bg_elements = create_background_and_borders(
                ctx.bounds,
                &self.style,
                block_start_y_in_ctx,
                (taken_height - vertical_spacing).max(0.0)
            );
            for el in bg_elements.into_iter().rev() {
                ctx.elements.insert(content_start_index, el);
            }

            ctx.cursor.1 = block_start_y_in_ctx + taken_height;
            let remaining_height = desired_border_box_height - taken_height;
            let mut next_style = (*self.style).clone();
            next_style.box_model.height = Some(Dimension::Pt(remaining_height));
            next_style.box_model.margin.top = 0.0;
            next_style.border.top = None;
            next_style.box_model.padding.top = 0.0;

            let remainder = BlockNode {
                id: self.id.clone(),
                children: vec![],
                style: Arc::new(next_style)
            };
            return Ok(LayoutResult::Partial(RenderNode::Block(remainder)));
        }

        // Full fit
        let final_content_height = if let Some(h) = fixed_height_opt {
            (h - vertical_spacing).max(0.0).max(content_height)
        } else {
            content_height
        };

        let bg_elements = create_background_and_borders(
            ctx.bounds,
            &self.style,
            block_start_y_in_ctx,
            final_content_height,
        );
        for el in bg_elements.into_iter().rev() {
            ctx.elements.insert(content_start_index, el);
        }

        ctx.cursor.1 = content_start_y_in_ctx
            + final_content_height
            + self.style.box_model.padding.bottom
            + border_bottom;
        ctx.last_v_margin = self.style.box_model.margin.bottom.max(child_last_v_margin);

        Ok(LayoutResult::Full)
    }

    fn check_for_page_break(&mut self) -> Option<Option<String>> {
        if let Some(first_child) = self.children.first_mut() {
            if first_child.is_page_break() {
                let page_break_node = self.children.remove(0);
                // We know it is a page break, extract safely
                if let RenderNode::PageBreak(node) = page_break_node {
                    return Some(node.master_name);
                }
            }
        }
        None
    }
}

pub(super) fn create_background_and_borders(
    bounds: geom::Rect,
    style: &Arc<ComputedStyle>,
    start_y: f32,
    content_height: f32,
) -> Vec<PositionedElement> {
    let mut elements = Vec::new();

    let border_top = style.border_top_width();
    let border_bottom = style.border_bottom_width();
    let border_left = style.border_left_width();
    let border_right = style.border_right_width();

    let inner_height = style.padding_y() + content_height;
    let total_height = border_top + inner_height + border_bottom;

    if total_height <= 0.0 {
        return elements;
    }

    let mut push = |mut element: PositionedElement, x: f32, y: f32| {
        element.x += bounds.x + x;
        element.y += bounds.y + y;
        elements.push(element);
    };

    if style.misc.background_color.is_some() {
        let mut bg_style = ComputedStyle::default();
        bg_style.misc.background_color = style.misc.background_color.clone();

        let bg_rect = geom::Rect {
            x: border_left,
            y: border_top,
            width: bounds.width - border_left - border_right,
            height: inner_height,
        };
        let bg = PositionedElement {
            element: LayoutElement::Rectangle(RectElement),
            style: Arc::new(bg_style),
            ..PositionedElement::from_rect(bg_rect)
        };
        push(bg, 0.0, start_y);
    }

    let mut draw_border = |b: &Option<Border>, rect: geom::Rect| {
        if let Some(border) = b {
            if border.width > 0.0 {
                let mut border_style = ComputedStyle::default();
                border_style.misc.background_color = Some(border.color.clone());

                let positioned_rect = PositionedElement {
                    element: LayoutElement::Rectangle(RectElement),
                    style: Arc::new(border_style),
                    ..PositionedElement::from_rect(rect)
                };
                push(positioned_rect, 0.0, start_y);
            }
        }
    };

    let bounds_width = bounds.width;
    draw_border(
        &style.border.top,
        geom::Rect {
            x: 0.0,
            y: 0.0,
            width: bounds_width,
            height: border_top,
        },
    );
    draw_border(
        &style.border.bottom,
        geom::Rect {
            x: 0.0,
            y: total_height - border_bottom,
            width: bounds_width,
            height: border_bottom,
        },
    );
    draw_border(
        &style.border.left,
        geom::Rect {
            x: 0.0,
            y: 0.0,
            width: border_left,
            height: total_height,
        },
    );
    draw_border(
        &style.border.right,
        geom::Rect {
            x: bounds_width - border_right,
            y: 0.0,
            width: border_right,
            height: total_height,
        },
    );

    elements
}

pub(super) fn draw_background_and_borders(
    elements: &mut Vec<PositionedElement>,
    bounds: geom::Rect,
    style: &Arc<ComputedStyle>,
    start_y: f32,
    content_height: f32,
) {
    let new_els = create_background_and_borders(bounds, style, start_y, content_height);
    elements.extend(new_els);
}