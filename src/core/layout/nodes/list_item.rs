use crate::core::layout::node::{AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode};
use crate::core::layout::nodes::block::draw_background_and_borders;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::util::VerticalStacker;
use crate::core::layout::{geom, LayoutElement, LayoutEngine, LayoutError, PositionedElement, TextElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::list::{ListStylePosition, ListStyleType};
use crate::core::style::text::TextDecoration;
use std::sync::Arc;
use crate::core::idf::IRNode;
use crate::core::layout::geom::{BoxConstraints, Size};

#[derive(Debug, Clone)]
pub struct ListItemNode {
    id: Option<String>,
    children: Vec<RenderNode>,
    style: Arc<ComputedStyle>,
    marker_text: String,
}

impl ListItemNode {
    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        index: usize,
        depth: usize,
    ) -> Result<Self, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (meta, ir_children) = match node {
            IRNode::ListItem { meta, children } => (meta, children),
            _ => return Err(LayoutError::BuilderMismatch("ListItem", node.kind())),
        };

        let mut children = Vec::new();
        for child_ir in ir_children {
            if let IRNode::List { .. } = child_ir {
                children.push(RenderNode::List(super::list::ListNode::new_with_depth(
                    child_ir,
                    engine,
                    style.clone(),
                    depth + 1,
                )?));
            } else {
                children.push(engine.build_layout_node_tree(child_ir, style.clone())?);
            }
        }

        let marker_text = get_marker_text(&style, index, depth);

        if style.list.style_position == ListStylePosition::Inside && !marker_text.is_empty() {
            if let Some(RenderNode::Paragraph(p_node)) = children.first_mut() {
                let mut new_p_node = p_node.clone();
                new_p_node.prepend_text(&format!("{} ", marker_text), engine);
                *p_node = new_p_node;
            }
        }

        Ok(Self {
            id: meta.id.clone(),
            children,
            style,
            marker_text,
        })
    }
}

fn get_marker_text(style: &Arc<ComputedStyle>, index: usize, depth: usize) -> String {
    let list_type_to_use = if depth > 0 && style.list.style_type == ListStyleType::Decimal {
        match depth % 3 {
            1 => &ListStyleType::LowerAlpha,
            2 => &ListStyleType::LowerRoman,
            _ => &ListStyleType::Decimal,
        }
    } else {
        &style.list.style_type
    };

    match list_type_to_use {
        ListStyleType::Disc => "•".to_string(),
        ListStyleType::Circle => "◦".to_string(),
        ListStyleType::Square => "▪".to_string(),
        ListStyleType::Decimal => format!("{}.", index),
        ListStyleType::LowerAlpha => format!("{}.", int_to_lower_alpha(index)),
        ListStyleType::UpperAlpha => format!("{}.", int_to_upper_alpha(index)),
        ListStyleType::LowerRoman => format!("{}.", int_to_lower_roman(index)),
        ListStyleType::UpperRoman => format!("{}.", int_to_upper_roman(index)),
        ListStyleType::None => String::new(),
    }
}

impl LayoutNode for ListItemNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let border_left = self.style.border_left_width();
        let border_right = self.style.border_right_width();
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;
        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            env.engine.measure_text_width(&self.marker_text, &self.style) + self.style.text.font_size * MARKER_SPACING_FACTOR
        } else {
            0.0
        };

        let child_constraints = if constraints.has_bounded_width() {
            let w = (constraints.max_width
                - self.style.box_model.padding.left
                - self.style.box_model.padding.right
                - border_left
                - border_right
                - indent).max(0.0);
            BoxConstraints {
                min_width: 0.0, max_width: w,
                min_height: 0.0, max_height: f32::INFINITY
            }
        } else {
            BoxConstraints {
                min_width: 0.0, max_width: f32::INFINITY,
                min_height: 0.0, max_height: f32::INFINITY
            }
        };

        let mut total_content_height = 0.0;
        for child in &mut self.children {
            total_content_height += child.measure(env, child_constraints).height;
        }

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();

        let height = if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            h
        } else {
            border_top
                + self.style.box_model.padding.top
                + total_content_height
                + self.style.box_model.padding.bottom
                + border_bottom
        };

        let width = if constraints.has_bounded_width() { constraints.max_width } else { 0.0 };

        Size::new(width, height)
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }

        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list.style_position == ListStylePosition::Outside;

        let block_start_y_in_ctx = ctx.cursor.1;

        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            let marker_available_height = self.style.text.line_height;
            if marker_available_height > ctx.available_height() && !ctx.is_empty() {
                return Ok(LayoutResult::Partial(RenderNode::ListItem(self.clone())));
            }

            let marker_width = ctx.engine.measure_text_width(&self.marker_text, &self.style);
            let marker_spacing = self.style.text.font_size * MARKER_SPACING_FACTOR;

            let marker_box = PositionedElement {
                x: 0.0,
                y: self.style.border_top_width() + self.style.box_model.padding.top,
                width: marker_width,
                height: self.style.text.line_height,
                element: LayoutElement::Text(TextElement {
                    content: self.marker_text.clone(),
                    href: None,
                    text_decoration: TextDecoration::None,
                }),
                style: self.style.clone(),
            };
            ctx.push_element_at(marker_box, 0.0, block_start_y_in_ctx);
            marker_width + marker_spacing
        } else {
            0.0
        };

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();
        let border_left = self.style.border_left_width();
        let _border_right = self.style.border_right_width();

        ctx.advance_cursor(border_top + self.style.box_model.padding.top);
        let content_start_y_in_ctx = ctx.cursor.1;

        let child_bounds = geom::Rect {
            x: ctx.bounds.x + border_left + self.style.box_model.padding.left + indent,
            y: ctx.bounds.y + content_start_y_in_ctx,
            width: ctx.bounds.width - self.style.padding_x() - self.style.border_x() - indent,
            height: ctx.available_height(),
        };

        let (split_result, content_height) = ctx.with_child_bounds(child_bounds, |child_ctx| {
            let result = VerticalStacker::layout_children(child_ctx, &mut self.children);
            (result, child_ctx.cursor.1)
        });

        // Unwrap logic result from closure
        let split_result = split_result?;

        if let Some(remaining_children) = split_result {
            draw_background_and_borders(
                ctx.elements,
                ctx.bounds,
                &self.style,
                block_start_y_in_ctx,
                content_height
            );
            ctx.cursor.1 = content_start_y_in_ctx + content_height + self.style.box_model.padding.bottom + border_bottom;

            let mut next_style = (*self.style).clone();
            next_style.box_model.margin.top = 0.0;
            next_style.border.top = None;
            next_style.box_model.padding.top = 0.0;
            if next_style.box_model.height.is_some() {
                next_style.box_model.height = None;
            }

            let mut next_page_item = ListItemNode {
                id: self.id.clone(),
                children: remaining_children,
                style: Arc::new(next_style),
                marker_text: String::new(),
            };
            next_page_item.measure(&LayoutEnvironment{ engine: ctx.engine, local_page_index: ctx.local_page_index }, BoxConstraints::tight_width(ctx.bounds.width));
            return Ok(LayoutResult::Partial(RenderNode::ListItem(next_page_item)));
        }

        draw_background_and_borders(
            ctx.elements,
            ctx.bounds,
            &self.style,
            block_start_y_in_ctx,
            content_height
        );

        ctx.cursor.1 = content_start_y_in_ctx + content_height + self.style.box_model.padding.bottom + border_bottom;

        Ok(LayoutResult::Full)
    }
}

fn int_to_lower_alpha(n: usize) -> String {
    if n == 0 { return "a".to_string(); }
    let mut s = String::new();
    let mut num = n - 1;
    loop {
        s.insert(0, (b'a' + (num % 26) as u8) as char);
        num /= 26;
        if num == 0 { break; }
        num -= 1;
    }
    s
}

fn int_to_upper_alpha(n: usize) -> String {
    int_to_lower_alpha(n).to_uppercase()
}

fn int_to_lower_roman(n: usize) -> String {
    if n == 0 { return String::new(); }
    let mut num = n;
    let values = [
        (1000, "m"), (900, "cm"), (500, "d"), (400, "cd"),
        (100, "c"), (90, "xc"), (50, "l"), (40, "xl"),
        (10, "x"), (9, "ix"), (5, "v"), (4, "iv"),
        (1, "i"),
    ];
    let mut result = String::new();
    for &(val, sym) in &values {
        while num >= val {
            result.push_str(sym);
            num -= val;
        }
    }
    result
}

fn int_to_upper_roman(n: usize) -> String {
    int_to_lower_roman(n).to_uppercase()
}