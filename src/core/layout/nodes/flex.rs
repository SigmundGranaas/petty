// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/flex.rs
use crate::core::idf::IRNode;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use std::sync::Arc;

#[derive(Debug)]
pub struct FlexNode {
    children: Vec<Box<dyn LayoutNode>>,
    style: Arc<ComputedStyle>,
    // Measurement results
    lines: Vec<FlexLine>,
}

impl FlexNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let ir_children = match node {
            IRNode::FlexContainer { children, .. } => children,
            _ => panic!("FlexNode must be created from an IRNode::FlexContainer"),
        };
        let children = ir_children
            .iter()
            .map(|c| engine.build_layout_node_tree(c, style.clone()))
            .collect();

        Self {
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

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        // First, let all children measure themselves.
        for child in &mut self.children {
            child.measure(engine, available_width);
        }
        // Now resolve the flex lines based on their measured sizes.
        self.lines = resolve_flex_lines(engine, &mut self.children, &self.style, available_width);
    }

    fn measure_content_height(&mut self, _engine: &LayoutEngine, _available_width: f32) -> f32 {
        self.lines.iter().map(|line| line.cross_size).sum()
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        let is_horiz = is_horizontal(&self.style.flex_direction);
        let container_main_size = if is_horiz { ctx.bounds.width } else { f32::INFINITY };

        let mut child_idx_offset = 0;

        for (line_idx, line) in self.lines.iter().enumerate() {
            if line.cross_size > ctx.available_height() && !ctx.is_empty() {
                let remaining_children = self.children.drain(child_idx_offset..).collect();
                let mut remaining_lines = self.lines.drain(line_idx..).collect::<Vec<_>>();

                // FIX: Re-index the items in the remaining lines to be relative to the new,
                // smaller children vector.
                for remaining_line in &mut remaining_lines {
                    for item in &mut remaining_line.items {
                        item.index -= child_idx_offset;
                    }
                }

                let remainder_node = FlexNode {
                    children: remaining_children,
                    style: self.style.clone(),
                    lines: remaining_lines,
                };
                return Ok(LayoutResult::Partial(Box::new(remainder_node)));
            }

            let free_space = container_main_size - line.main_size;
            let (mut main_cursor, spacing) = calculate_main_axis_alignment(
                free_space,
                line.items.len(),
                &self.style.justify_content,
            );

            for item in &line.items {
                let item_cross_offset =
                    calculate_cross_axis_alignment(item, line.cross_size, &self.style.align_items);

                let (x, y, width, height) = if is_horiz {
                    (main_cursor, item_cross_offset, item.main_size, item.cross_size)
                } else {
                    (item_cross_offset, main_cursor, item.cross_size, item.main_size)
                };

                let child_bounds = geom::Rect {
                    x: ctx.bounds.x + x,
                    y: ctx.bounds.y + ctx.cursor.1 + y,
                    width,
                    height,
                };

                let mut child_ctx = LayoutContext {
                    engine: ctx.engine,
                    bounds: child_bounds,
                    cursor: (0.0, 0.0),
                    elements: unsafe { &mut *(ctx.elements as *mut Vec<PositionedElement>) },
                };

                match self.children[item.index].layout(&mut child_ctx) {
                    Ok(_) => { /* Continue */ }
                    Err(e) => {
                        log::warn!("Skipping flex item that failed to lay out: {}", e);
                    }
                }
                main_cursor += item.main_size + spacing;
            }

            ctx.advance_cursor(line.cross_size);
            child_idx_offset += line.items.len();
        }

        Ok(LayoutResult::Full)
    }
}

// --- Flexbox Algorithm Internals ---

#[derive(Clone, Debug)]
struct FlexItem {
    index: usize,
    style: Arc<ComputedStyle>,
    flex_basis: f32,
    main_size: f32,
    cross_size: f32,
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

fn resolve_flex_lines(
    engine: &LayoutEngine,
    children: &mut [Box<dyn LayoutNode>],
    style: &Arc<ComputedStyle>,
    available_width: f32,
) -> Vec<FlexLine> {
    let is_horiz = is_horizontal(&style.flex_direction);
    let available_main_size = if is_horiz { available_width } else { f32::INFINITY };

    let items: Vec<FlexItem> = children
        .iter_mut()
        .enumerate()
        .map(|(i, child_node)| {
            let item_style = child_node.style().clone();

            let flex_basis =
                resolve_flex_basis(engine, child_node, &item_style, available_main_size, is_horiz);

            let cross_size = child_node.measure_content_height(
                engine,
                if is_horiz { flex_basis } else { available_width },
            );

            FlexItem {
                index: i,
                style: item_style,
                flex_basis,
                main_size: flex_basis,
                cross_size: if is_horiz { cross_size } else { flex_basis },
            }
        })
        .collect();

    // Line breaking logic
    let mut lines = Vec::new();
    let mut current_line_items = Vec::new();
    let mut current_line_main_size = 0.0;

    for item in items {
        if style.flex_wrap != FlexWrap::NoWrap
            && !current_line_items.is_empty()
            && current_line_main_size + item.main_size > available_main_size
        {
            lines.push(FlexLine {
                items: std::mem::take(&mut current_line_items),
                main_size: current_line_main_size,
                cross_size: 0.0,
            });
            current_line_main_size = 0.0;
        }
        current_line_main_size += item.main_size;
        current_line_items.push(item);
    }
    if !current_line_items.is_empty() {
        lines.push(FlexLine {
            items: current_line_items,
            main_size: current_line_main_size,
            cross_size: 0.0,
        });
    }

    if style.flex_wrap == FlexWrap::WrapReverse {
        lines.reverse();
    }

    // Resolve flexible lengths and final cross sizes for each line.
    for line in &mut lines {
        resolve_flexible_lengths(line, available_main_size);
        line.cross_size = line.items.iter().map(|i| i.cross_size).fold(0.0, f32::max);
    }

    lines
}

fn resolve_flex_basis(
    engine: &LayoutEngine,
    node: &mut Box<dyn LayoutNode>,
    style: &Arc<ComputedStyle>,
    container_main_size: f32,
    is_horiz: bool,
) -> f32 {
    let basis_prop = &style.flex_basis;
    let size_prop = if is_horiz { style.width.as_ref() } else { style.height.as_ref() };
    let resolved_basis_dim = match basis_prop {
        Dimension::Auto => size_prop,
        _ => Some(basis_prop),
    };
    match resolved_basis_dim {
        Some(Dimension::Pt(val)) => *val,
        Some(Dimension::Percent(p)) => container_main_size * (p / 100.0),
        _ => {
            // Auto basis means size is determined by content.
            if is_horiz {
                // This is an approximation. True horizontal content sizing is complex.
                // We'll use the child's measured height as a rough guide for width.
                node.measure_content_height(engine, f32::INFINITY)
            } else {
                node.measure_content_height(engine, container_main_size)
            }
        }
    }
}

fn resolve_flexible_lengths(line: &mut FlexLine, available_main_size: f32) {
    let initial_main_size: f32 = line.items.iter().map(|i| i.main_size).sum();
    let remaining_space = available_main_size - initial_main_size;
    if remaining_space > 0.0 {
        let total_grow: f32 = line.items.iter().map(|i| i.style.flex_grow).sum();
        if total_grow > 0.0 {
            for item in &mut line.items {
                item.main_size += remaining_space * (item.style.flex_grow / total_grow);
            }
        }
    } else if remaining_space < 0.0 {
        let total_shrink: f32 = line
            .items
            .iter()
            .map(|i| i.style.flex_shrink * i.flex_basis)
            .sum();
        if total_shrink > 0.0 {
            for item in &mut line.items {
                let shrink_ratio = (item.style.flex_shrink * item.flex_basis) / total_shrink;
                item.main_size += remaining_space * shrink_ratio;
            }
        }
    }
    line.main_size = line.items.iter().map(|i| i.main_size).sum();
}

fn calculate_main_axis_alignment(
    free_space: f32,
    item_count: usize,
    justify: &JustifyContent,
) -> (f32, f32) {
    if free_space <= 0.0 { return (0.0, 0.0); }
    match justify {
        JustifyContent::FlexStart => (0.0, 0.0),
        JustifyContent::FlexEnd => (free_space, 0.0),
        JustifyContent::Center => (free_space / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if item_count > 1 { (0.0, free_space / (item_count - 1) as f32) } else { (free_space / 2.0, 0.0) }
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
    let align = match &item.style.align_self {
        AlignSelf::Auto => container_align,
        AlignSelf::Stretch => &AlignItems::Stretch,
        AlignSelf::FlexStart => &AlignItems::FlexStart,
        AlignSelf::FlexEnd => &AlignItems::FlexEnd,
        AlignSelf::Center => &AlignItems::Center,
        AlignSelf::Baseline => &AlignItems::Baseline,
    };
    match align {
        AlignItems::Stretch | AlignItems::FlexStart | AlignItems::Baseline => 0.0,
        AlignItems::FlexEnd => line_cross_size - item.cross_size,
        AlignItems::Center => (line_cross_size - item.cross_size) / 2.0,
    }
}