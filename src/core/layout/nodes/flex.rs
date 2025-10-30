use crate::core::idf::IRNode;
use crate::core::layout::node::{AnchorLocation, LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::block::draw_background_and_borders;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FlexNode {
    id: Option<String>,
    children: Vec<Box<dyn LayoutNode>>,
    style: Arc<ComputedStyle>,
    lines: Vec<FlexLine>,
}

impl FlexNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (id, ir_children) = match node {
            IRNode::FlexContainer { meta, children } => (meta.id.clone(), children),
            _ => panic!("FlexNode must be created from an IRNode::FlexContainer"),
        };
        let children = ir_children
            .iter()
            .map(|c| engine.build_layout_node_tree(c, style.clone()))
            .collect();

        Self {
            id,
            children,
            style,
            lines: vec![],
        }
    }
}

impl LayoutNode for FlexNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);
        let child_available_width = available_width
            - self.style.padding.left
            - self.style.padding.right
            - border_left_width
            - border_right_width;

        for child in &mut self.children {
            child.measure(engine, child_available_width);
        }
        self.lines = resolve_flex_lines(engine, &mut self.children, &self.style, child_available_width);
    }

    fn measure_content_height(&mut self, _engine: &LayoutEngine, _available_width: f32) -> f32 {
        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let content_height: f32 = self.lines.iter().map(|line| line.cross_size).sum();

        self.style.margin.top
            + border_top_width
            + self.style.padding.top
            + content_height
            + self.style.padding.bottom
            + border_bottom_width
            + self.style.margin.bottom
    }

    fn measure_intrinsic_width(&self, engine: &LayoutEngine) -> f32 {
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);
        let own_width =
            self.style.padding.left + self.style.padding.right + border_left_width + border_right_width;

        let is_horiz = is_horizontal(&self.style.flex_direction);
        if is_horiz {
            // For a row, the intrinsic width is the sum of the children's intrinsic widths.
            let children_width: f32 = self.children.iter().map(|c| c.measure_intrinsic_width(engine)).sum();
            children_width + own_width
        } else {
            // For a column, it's the max of the children's intrinsic widths.
            let children_width: f32 = self
                .children
                .iter()
                .map(|c| c.measure_intrinsic_width(engine))
                .fold(0.0, f32::max);
            children_width + own_width
        }
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.borrow_mut().insert(id.clone(), location);
        }

        // --- Box Model Setup ---
        let margin_to_add = self.style.margin.top.max(ctx.last_v_margin);
        if !ctx.is_empty() && margin_to_add > ctx.available_height() {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }
        ctx.advance_cursor(margin_to_add);
        ctx.last_v_margin = 0.0;

        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        let block_start_y_in_ctx = ctx.cursor.1;
        ctx.advance_cursor(border_top_width + self.style.padding.top);
        let content_start_y_in_ctx = ctx.cursor.1;

        // --- Flex Layout Logic ---
        let child_bounds = geom::Rect {
            x: ctx.bounds.x + border_left_width + self.style.padding.left,
            y: ctx.bounds.y + content_start_y_in_ctx,
            width: ctx.bounds.width
                - self.style.padding.left
                - self.style.padding.right
                - border_left_width
                - border_right_width,
            height: ctx.available_height(),
        };

        let is_horiz = is_horizontal(&self.style.flex_direction);
        let is_reverse = is_main_reverse(&self.style.flex_direction);
        let container_main_size = child_bounds.width; // Use content-box width
        let container_cross_size = child_bounds.height;

        let total_lines_cross_size: f32 = self.lines.iter().map(|line| line.cross_size).sum();
        let free_cross_space = container_cross_size - total_lines_cross_size;
        let (mut cross_cursor, line_spacing) =
            calculate_main_axis_alignment(free_cross_space, self.lines.len(), &self.style.align_content);

        let mut child_idx_offset = 0;
        let mut lines_on_this_page = 0;
        let mut content_height_on_this_page = 0.0;

        for line in &self.lines {
            if line.cross_size > (child_bounds.height - content_height_on_this_page) && lines_on_this_page > 0 {
                let remaining_children = self.children.drain(child_idx_offset..).collect();
                draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height_on_this_page);
                ctx.cursor.1 = content_start_y_in_ctx
                    + content_height_on_this_page
                    + self.style.padding.bottom
                    + border_bottom_width;
                // Since flex items don't have collapsing margins with the container, last_v_margin is not set from children.

                let mut remainder_node = FlexNode {
                    id: self.id.clone(),
                    children: remaining_children,
                    style: self.style.clone(),
                    lines: vec![], // Will be recalculated
                };
                remainder_node.measure(ctx.engine, ctx.bounds.width);
                return Ok(LayoutResult::Partial(Box::new(remainder_node)));
            }
            lines_on_this_page += 1;
            content_height_on_this_page += line.cross_size;

            let mut effective_justify = self.style.justify_content.clone();
            if is_reverse {
                effective_justify = match effective_justify {
                    JustifyContent::FlexStart => JustifyContent::FlexEnd,
                    JustifyContent::FlexEnd => JustifyContent::FlexStart,
                    _ => effective_justify,
                };
            }

            let free_space = container_main_size - line.main_size;
            let (mut main_cursor, item_spacing) =
                calculate_main_axis_alignment(free_space, line.items.len(), &effective_justify);

            for item in &line.items {
                main_cursor += if is_reverse { item.main_margin_end } else { item.main_margin_start };
                let cross_offset_within_line = calculate_cross_axis_alignment(item, line.cross_size, &self.style.align_items);
                let item_cross_offset = cross_cursor + cross_offset_within_line + if is_cross_reverse(&self.style.flex_wrap) {
                    item.cross_margin_end
                } else {
                    item.cross_margin_start
                };

                let (x, y, width, height) = if is_horiz {
                    (main_cursor, item_cross_offset, item.main_size, item.cross_size)
                } else {
                    (item_cross_offset, main_cursor, item.cross_size, item.main_size)
                };

                let item_bounds = geom::Rect {
                    x: child_bounds.x + x,
                    y: child_bounds.y + y,
                    width,
                    height,
                };

                let mut item_ctx = LayoutContext {
                    engine: ctx.engine,
                    bounds: item_bounds,
                    cursor: (0.0, 0.0),
                    elements: ctx.elements,
                    last_v_margin: 0.0,
                    local_page_index: ctx.local_page_index,
                    defined_anchors: ctx.defined_anchors,
                };

                match self.children[item.original_index].layout(&mut item_ctx) {
                    Ok(_) => {}
                    Err(e) => log::warn!("Skipping flex item that failed to lay out: {}", e),
                }
                main_cursor += item.main_size + item_spacing + if is_reverse { item.main_margin_start } else { item.main_margin_end };
            }
            cross_cursor += line.cross_size + line_spacing;
            child_idx_offset += line.items.len();
        }

        draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height_on_this_page);
        ctx.cursor.1 = content_start_y_in_ctx
            + content_height_on_this_page
            + self.style.padding.bottom
            + border_bottom_width;
        ctx.last_v_margin = self.style.margin.bottom;
        Ok(LayoutResult::Full)
    }
}

// --- Flexbox Algorithm Internals ---

#[derive(Clone, Debug)]
struct FlexItem {
    original_index: usize,
    style: Arc<ComputedStyle>,
    flex_basis: f32,
    main_size: f32,
    cross_size: f32,
    main_margin_start: f32,
    main_margin_end: f32,
    cross_margin_start: f32,
    cross_margin_end: f32,
}

#[derive(Clone, Debug)]
struct FlexLine {
    items: Vec<FlexItem>,
    main_size: f32,
    cross_size: f32,
}

fn is_horizontal(direction: &FlexDirection) -> bool {
    matches!(direction, FlexDirection::Row | FlexDirection::RowReverse)
}

fn is_main_reverse(direction: &FlexDirection) -> bool {
    matches!(direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse)
}

fn is_cross_reverse(wrap: &FlexWrap) -> bool {
    matches!(wrap, FlexWrap::WrapReverse)
}

fn resolve_flex_lines(
    engine: &LayoutEngine,
    children: &mut [Box<dyn LayoutNode>],
    style: &Arc<ComputedStyle>,
    available_width: f32,
) -> Vec<FlexLine> {
    let is_horiz = is_horizontal(&style.flex_direction);
    let available_main_size = if is_horiz { available_width } else { f32::INFINITY };

    let mut items: Vec<FlexItem> = children
        .iter_mut()
        .enumerate()
        .map(|(i, child_node)| {
            let item_style = child_node.style().clone();
            let flex_basis = resolve_flex_basis(engine, child_node.as_mut(), &item_style, available_main_size, is_horiz);
            let cross_size = child_node.measure_content_height(engine, if is_horiz { flex_basis } else { available_width });

            let (main_margin_start, main_margin_end, cross_margin_start, cross_margin_end) = if is_horiz {
                (item_style.margin.left, item_style.margin.right, item_style.margin.top, item_style.margin.bottom)
            } else {
                (item_style.margin.top, item_style.margin.bottom, item_style.margin.left, item_style.margin.right)
            };

            FlexItem {
                original_index: i,
                style: item_style,
                flex_basis,
                main_size: flex_basis,
                cross_size: if is_horiz { cross_size } else { flex_basis },
                main_margin_start,
                main_margin_end,
                cross_margin_start,
                cross_margin_end,
            }
        })
        .collect();

    items.sort_by_key(|item| item.style.order);
    if is_main_reverse(&style.flex_direction) {
        items.reverse();
    }

    let mut lines = Vec::new();
    let mut current_line_items = Vec::new();
    let mut current_line_main_size = 0.0;

    for item in items {
        let item_total_main = item.main_size + item.main_margin_start + item.main_margin_end;
        if style.flex_wrap != FlexWrap::NoWrap
            && !current_line_items.is_empty()
            && current_line_main_size + item_total_main > available_main_size
        {
            lines.push(FlexLine {
                items: std::mem::take(&mut current_line_items),
                main_size: current_line_main_size,
                cross_size: 0.0,
            });
            current_line_main_size = 0.0;
        }
        current_line_main_size += item_total_main;
        current_line_items.push(item);
    }
    if !current_line_items.is_empty() {
        lines.push(FlexLine {
            items: current_line_items,
            main_size: current_line_main_size,
            cross_size: 0.0,
        });
    }

    if is_cross_reverse(&style.flex_wrap) {
        lines.reverse();
    }

    for line in &mut lines {
        resolve_flexible_lengths(line, available_main_size);
        let max_cross_margin = line
            .items
            .iter()
            .map(|i| i.cross_margin_start + i.cross_margin_end)
            .fold(0.0, f32::max);
        line.cross_size = line.items.iter().map(|i| i.cross_size).fold(0.0, f32::max) + max_cross_margin;
    }
    lines
}

fn resolve_flex_basis(
    engine: &LayoutEngine,
    node: &mut dyn LayoutNode,
    style: &Arc<ComputedStyle>,
    container_main_size: f32,
    is_horiz: bool,
) -> f32 {
    let basis_prop = &style.flex_basis;
    let size_prop = if is_horiz { &style.width } else { &style.height };

    let resolved_basis_dim = if style.flex_basis == Dimension::Auto {
        size_prop.as_ref().or(Some(basis_prop))
    } else {
        Some(basis_prop)
    };

    match resolved_basis_dim {
        Some(Dimension::Pt(val)) => *val,
        Some(Dimension::Percent(p)) => container_main_size * (p / 100.0),
        _ => { // This is Dimension::Auto
            if is_horiz {
                // For 'auto', we need the intrinsic width of the content.
                node.measure_intrinsic_width(engine)
            } else {
                // For vertical flex, we need the intrinsic height.
                node.measure_content_height(engine, container_main_size)
            }
        }
    }
}

fn resolve_flexible_lengths(line: &mut FlexLine, available_main_size: f32) {
    let initial_main_size: f32 = line
        .items
        .iter()
        .map(|i| i.main_size + i.main_margin_start + i.main_margin_end)
        .sum();
    let remaining_space = available_main_size - initial_main_size;

    if remaining_space.abs() < 0.1 {
        line.main_size = initial_main_size;
        return;
    }

    if remaining_space > 0.0 {
        let total_grow: f32 = line.items.iter().map(|i| i.style.flex_grow).sum();
        if total_grow > 0.0 {
            for item in &mut line.items {
                if item.style.flex_grow > 0.0 {
                    item.main_size += remaining_space * (item.style.flex_grow / total_grow);
                }
            }
        }
    } else if remaining_space < 0.0 {
        let total_shrink: f32 = line.items.iter().map(|i| i.style.flex_shrink * i.flex_basis).sum();
        if total_shrink > 0.0 {
            for item in &mut line.items {
                if item.style.flex_shrink > 0.0 {
                    let shrink_ratio = (item.style.flex_shrink * item.flex_basis) / total_shrink;
                    item.main_size += remaining_space * shrink_ratio;
                }
            }
        }
    }
    line.main_size = line
        .items
        .iter()
        .map(|i| i.main_size + i.main_margin_start + i.main_margin_end)
        .sum();
}

fn calculate_main_axis_alignment(
    free_space: f32,
    item_count: usize,
    justify: &JustifyContent,
) -> (f32, f32) {
    if free_space <= 0.0 || item_count == 0 {
        return (0.0, 0.0);
    }
    match justify {
        JustifyContent::FlexStart => (0.0, 0.0),
        JustifyContent::FlexEnd => (free_space, 0.0),
        JustifyContent::Center => (free_space / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if item_count > 1 {
                (0.0, free_space / (item_count - 1) as f32)
            } else {
                (free_space / 2.0, 0.0)
            }
        }
        JustifyContent::SpaceAround => {
            let spacing = free_space / item_count as f32;
            (spacing / 2.0, spacing)
        }
        JustifyContent::SpaceEvenly => {
            let spacing = free_space / (item_count + 1) as f32;
            (spacing, spacing)
        }
    }
}

fn calculate_cross_axis_alignment(item: &FlexItem, line_cross_size: f32, container_align: &AlignItems) -> f32 {
    let item_total_cross_size = item.cross_size + item.cross_margin_start + item.cross_margin_end;
    let align = match &item.style.align_self {
        AlignSelf::Auto => container_align,
        AlignSelf::Stretch => &AlignItems::Stretch,
        AlignSelf::FlexStart => &AlignItems::FlexStart,
        AlignSelf::FlexEnd => &AlignItems::FlexEnd,
        AlignSelf::Center => &AlignItems::Center,
        AlignSelf::Baseline => {
            log::warn!("align-self: baseline is not supported, falling back to flex-start");
            &AlignItems::FlexStart
        }
    };
    match align {
        AlignItems::Stretch => {
            // Stretching is handled by the parent giving the child the full cross size.
            // Here, we just align to the start.
            0.0
        }
        AlignItems::FlexStart | AlignItems::Baseline => 0.0,
        AlignItems::FlexEnd => line_cross_size - item_total_cross_size,
        AlignItems::Center => (line_cross_size - item_total_cross_size) / 2.0,
    }
}