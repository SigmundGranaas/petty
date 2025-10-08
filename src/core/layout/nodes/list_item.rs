// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/list_item.rs
use crate::core::idf::IRNode;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::block::draw_background_and_borders;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutElement, LayoutEngine, LayoutError, PositionedElement, TextElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::list::{ListStylePosition, ListStyleType};
use crate::core::style::text::TextDecoration;
use std::any::Any;
use std::sync::Arc;

/// A `LayoutNode` for a single item within a list.
/// It is responsible for drawing its marker (bullet or number) and then
/// laying out its own children in an indented area.
#[derive(Debug, Clone)]
pub struct ListItemNode {
    children: Vec<Box<dyn LayoutNode>>,
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
    ) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let ir_children = match node {
            IRNode::ListItem { children, .. } => children,
            _ => panic!("ListItemNode must be created from an IRNode::ListItem"),
        };

        // When a list item itself contains a list, we must create that child
        // ListNode with an incremented depth. The generic `build_layout_node_tree`
        // does not know about list depth, so we handle that case specifically here.
        let mut children: Vec<Box<dyn LayoutNode>> = ir_children
            .iter()
            .map(|child_ir| {
                // Special handling for nested lists to pass depth correctly
                if let IRNode::List { .. } = child_ir {
                    Box::new(super::list::ListNode::new_with_depth(
                        child_ir,
                        engine,
                        style.clone(),
                        depth + 1, // Pass the incremented depth
                    )) as Box<dyn LayoutNode>
                } else {
                    engine.build_layout_node_tree(child_ir, style.clone())
                }
            })
            .collect();

        // Determine marker content based on style and depth.
        let marker_text = get_marker_text(&style, index, depth);

        // For 'inside' positioning, prepend the marker to the first paragraph.
        // This does not currently support image markers.
        if style.list_style_position == ListStylePosition::Inside && !marker_text.is_empty() {
            if let Some(first_child) = children.first_mut() {
                if let Some(p_node) = first_child.as_any().downcast_ref::<super::paragraph::ParagraphNode>() {
                    let mut new_p_node = p_node.clone();
                    new_p_node.prepend_text(&format!("{} ", marker_text), engine);
                    *first_child = Box::new(new_p_node);
                }
            }
        }

        Self {
            children,
            style,
            marker_text,
        }
    }
}

fn get_marker_text(style: &Arc<ComputedStyle>, index: usize, depth: usize) -> String {
    let list_type_to_use = if depth > 0 && style.list_style_type == ListStyleType::Decimal {
        // If it's a nested decimal list, cycle through styles for convenience.
        // An explicitly styled nested list will not enter this branch.
        match depth % 3 {
            1 => &ListStyleType::LowerAlpha,
            2 => &ListStyleType::LowerRoman,
            _ => &ListStyleType::Decimal, // for depth 3, 6, etc.
        }
    } else {
        // Otherwise, use the style specified for this list level.
        &style.list_style_type
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        // List items behave like blocks; their children's available width is reduced
        // by the list item's own padding and borders.
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list_style_position == ListStylePosition::Outside;
        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            engine.measure_text_width(&self.marker_text, &self.style) + self.style.font_size * MARKER_SPACING_FACTOR
        } else {
            0.0
        };

        let child_available_width = available_width
            - self.style.padding.left
            - self.style.padding.right
            - border_left_width
            - border_right_width
            - indent;

        for child in &mut self.children {
            child.measure(engine, child_available_width);
        }
    }
    fn measure_content_height(&mut self, engine: &LayoutEngine, available_width: f32) -> f32 {
        if let Some(Dimension::Pt(h)) = self.style.height {
            return h;
        }
        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);

        let content_height: f32 = self
            .children
            .iter_mut()
            .map(|c| c.measure_content_height(engine, available_width))
            .sum();

        border_top_width
            + self.style.padding.top
            + content_height
            + self.style.padding.bottom
            + border_bottom_width
    }
    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        const MARKER_SPACING_FACTOR: f32 = 0.4;
        let is_outside_marker = self.style.list_style_position == ListStylePosition::Outside;

        let block_start_y_in_ctx = ctx.cursor.1;

        // --- 1. Handle Marker Layout for 'outside' markers ---
        // This happens before the box model is applied to the content.
        let indent = if is_outside_marker && !self.marker_text.is_empty() {
            let marker_available_height = self.style.line_height;
            if marker_available_height > ctx.available_height() && !ctx.is_empty() {
                return Ok(LayoutResult::Partial(Box::new(self.clone())));
            }

            let marker_width = ctx.engine.measure_text_width(&self.marker_text, &self.style);
            let marker_spacing = self.style.font_size * MARKER_SPACING_FACTOR;

            let marker_box = PositionedElement {
                x: 0.0,
                y: self.style.border_top.as_ref().map_or(0.0, |b| b.width) + self.style.padding.top,
                width: marker_width,
                height: self.style.line_height,
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

        // --- 2. Layout Children within a proper box model ---
        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        ctx.advance_cursor(border_top_width + self.style.padding.top);
        let content_start_y_in_ctx = ctx.cursor.1;

        let child_bounds = geom::Rect {
            x: ctx.bounds.x + border_left_width + self.style.padding.left + indent,
            y: ctx.bounds.y + content_start_y_in_ctx,
            width: ctx.bounds.width - self.style.padding.left - self.style.padding.right - border_left_width - border_right_width - indent,
            height: ctx.available_height(),
        };

        let mut child_ctx = LayoutContext {
            engine: ctx.engine,
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: ctx.elements,
            last_v_margin: 0.0, // List items create a new block formatting context
        };

        for (i, child) in self.children.iter_mut().enumerate() {
            match child.layout(&mut child_ctx)? {
                LayoutResult::Full => continue,
                LayoutResult::Partial(remainder) => {
                    let content_height = child_ctx.cursor.1;
                    draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height);
                    ctx.cursor.1 = content_start_y_in_ctx + content_height + self.style.padding.bottom + border_bottom_width;

                    let mut remaining_children = vec![remainder];
                    remaining_children.extend(self.children.drain((i + 1)..));

                    let next_page_item = Box::new(ListItemNode {
                        children: remaining_children,
                        style: self.style.clone(),
                        marker_text: String::new(), // No marker on subsequent pages
                    });
                    return Ok(LayoutResult::Partial(next_page_item));
                }
            }
        }

        let content_height = child_ctx.cursor.1;
        draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height);
        ctx.cursor.1 = content_start_y_in_ctx + content_height + self.style.padding.bottom + border_bottom_width;

        Ok(LayoutResult::Full)
    }
}

// Helper functions for list numbering
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