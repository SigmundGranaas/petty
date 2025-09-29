// src/core/layout/flex.rs

use super::style::ComputedStyle;
use super::{image, text, IRNode, LayoutBox, LayoutContent, LayoutEngine, Rect};
use crate::core::style::dimension::Dimension;
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use std::sync::Arc;

/// An intermediate representation of a flex item during layout calculations.
#[derive(Clone)]
struct FlexItem {
    index: usize,
    style: Arc<ComputedStyle>,
    // The initial main size of the item, resolved from width/height/flex-basis.
    flex_basis: f32,
    // The item's final calculated size on the main axis.
    main_size: f32,
    // The item's final calculated size on the cross axis.
    cross_size: f32,
    // The fully laid out LayoutBox for the item's content.
    layout_box: Option<LayoutBox>,
}

/// Represents a single line of items in a flex container.
struct FlexLine {
    items: Vec<FlexItem>,
    // The sum of the main_size of all items in the line.
    main_size: f32,
    // The maximum cross_size of all items in the line.
    cross_size: f32,
}

/// Determines if the main axis is horizontal based on flex-direction.
fn is_horizontal(direction: &FlexDirection) -> bool {
    matches!(direction, FlexDirection::Row | FlexDirection::RowReverse)
}

/// Lays out a flex container. This is the heart of the Flexbox implementation.
pub(super) fn layout_flex_container(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: Arc<ComputedStyle>,
    available_size: (f32, f32),
) -> LayoutBox {
    let children = match node {
        IRNode::FlexContainer { children, .. } => children,
        _ => {
            return LayoutBox {
                rect: Rect::default(),
                style,
                content: LayoutContent::Children(vec![]),
            }
        }
    };

    let (mut lines, total_cross_size, total_main_size) =
        resolve_flex_lines(engine, children, &style, available_size);

    let mut all_child_boxes = Vec::new();
    let is_horiz = is_horizontal(&style.flex_direction);
    let is_reverse = matches!(
        style.flex_direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
    );

    let container_main_size = if is_horiz {
        available_size.0
    } else {
        total_main_size
    };

    // Position the items line by line.
    let mut cross_cursor = 0.0;
    for line in &mut lines {
        // Step 1: Handle `justify-content`
        let free_space = container_main_size - line.main_size;
        let (mut main_cursor, spacing) =
            calculate_main_axis_alignment(free_space, line.items.len(), &style.justify_content);

        // Step 2: Iterate through items to position them.
        for item in &mut line.items {
            // Step 2a: Handle `align-items` and `align-self` for cross-axis position.
            let item_cross_offset =
                calculate_cross_axis_alignment(item, line.cross_size, &style.align_items);

            // Step 2b: Calculate final position
            let final_main_pos = if is_reverse {
                container_main_size - main_cursor - item.main_size
            } else {
                main_cursor
            };
            let final_cross_pos = cross_cursor + item_cross_offset;

            // Step 2c: Position the item's LayoutBox
            if let Some(layout_box) = item.layout_box.as_mut() {
                if is_horiz {
                    layout_box.rect.x = final_main_pos;
                    layout_box.rect.y = final_cross_pos;
                } else {
                    layout_box.rect.x = final_cross_pos;
                    layout_box.rect.y = final_main_pos;
                }
                all_child_boxes.push(layout_box.clone());
            }
            main_cursor += item.main_size + spacing;
        }
        cross_cursor += line.cross_size;
    }

    let final_container_height = if is_horiz {
        total_cross_size
    } else {
        total_main_size
    };

    LayoutBox {
        rect: Rect {
            height: final_container_height,
            ..Default::default()
        },
        style,
        content: LayoutContent::Children(all_child_boxes),
    }
}

/// Performs the core flexbox algorithm: sizing items and breaking them into lines.
fn resolve_flex_lines(
    engine: &LayoutEngine,
    children: &mut [IRNode],
    style: &Arc<ComputedStyle>,
    available_size: (f32, f32),
) -> (Vec<FlexLine>, f32, f32) {
    let is_horiz = is_horizontal(&style.flex_direction);
    let available_main_size = if is_horiz {
        available_size.0
    } else {
        f32::INFINITY
    };

    // Step 1: Collect all children into `FlexItem`s and compute their initial styles and basis.
    let items: Vec<FlexItem> = children
        .iter_mut()
        .enumerate()
        .map(|(i, child_node)| {
            let item_style =
                engine.compute_style(child_node.style_sets(), child_node.style_override(), style);
            let flex_basis =
                resolve_flex_basis(engine, child_node, &item_style, available_main_size, is_horiz);
            FlexItem {
                index: i,
                style: item_style,
                flex_basis,
                main_size: flex_basis,
                cross_size: 0.0,
                layout_box: None,
            }
        })
        .collect();

    // Step 2: Partition items into flex lines.
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

    // Step 3: For each line, resolve flexible lengths and determine cross sizes.
    for line in &mut lines {
        resolve_flexible_lengths(line, available_main_size);

        let mut max_cross_size = 0.0f32;
        for item in &mut line.items {
            let child_node = &mut children[item.index];
            let item_available_size = if is_horiz {
                (item.main_size, available_size.1)
            } else {
                (available_size.0, item.main_size)
            };

            let mut layout_box =
                engine.build_layout_tree(child_node, item.style.clone(), item_available_size);

            // The flex algorithm is the authority on the main size. Override whatever
            // the recursive call to build_layout_tree produced.
            if is_horiz {
                layout_box.rect.width = item.main_size;
                item.cross_size = layout_box.rect.height;
            } else {
                layout_box.rect.height = item.main_size;
                item.cross_size = layout_box.rect.width;
            }

            item.layout_box = Some(layout_box);
            max_cross_size = max_cross_size.max(item.cross_size);
        }
        line.cross_size = max_cross_size;
    }

    let total_cross_size: f32 = lines.iter().map(|l| l.cross_size).sum();
    let total_main_size: f32 = if is_horiz {
        available_main_size
    } else {
        lines.iter().map(|l| l.main_size).sum()
    };

    (lines, total_cross_size, total_main_size)
}

/// Determines the initial main size of a flex item (the "flex basis").
fn resolve_flex_basis(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    container_main_size: f32,
    is_horiz: bool,
) -> f32 {
    let basis_prop = &style.flex_basis;
    let size_prop = if is_horiz {
        style.width.as_ref()
    } else {
        style.height.as_ref()
    };

    let resolved_basis_dim = match basis_prop {
        Dimension::Auto => size_prop,
        _ => Some(basis_prop),
    };

    match resolved_basis_dim {
        Some(Dimension::Pt(val)) => *val,
        Some(Dimension::Percent(p)) => container_main_size * (p / 100.0),
        _ => {
            // "Auto" or `None` means shrink-to-fit sizing.
            if is_horiz {
                measure_max_content_width(engine, node, style, container_main_size)
            } else {
                // Height depends on width, so we lay it out with the container's width.
                let layout_box = engine.build_layout_tree(
                    node,
                    style.clone(),
                    (container_main_size, f32::INFINITY),
                );
                layout_box.rect.height
            }
        }
    }
}

/// Measures the max-content width of a node (for `width: auto` in a flex context).
fn measure_max_content_width(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    available_width: f32,
) -> f32 {
    match node {
        IRNode::Paragraph { .. } => text::measure_paragraph_max_content_width(engine, node, style),
        IRNode::Image { .. } => image::layout_image(node, style.clone(), (available_width, f32::INFINITY)).rect.width,
        IRNode::Block { children, .. } | IRNode::ListItem { children, .. } => children
            .iter_mut()
            .map(|c| measure_max_content_width(engine, c, style, available_width))
            .fold(0.0, f32::max),
        IRNode::FlexContainer { .. } => {
            // For nested flex containers, max-content width is complex.
            // As a simplification, we lay it out and measure its resulting width.
            layout_flex_container(engine, node, style.clone(), (available_width, f32::INFINITY)).rect.width
        }
        _ => 0.0, // Simplification for other types like Table, List
    }
}


/// Applies flex-grow and flex-shrink to items within a single line.
fn resolve_flexible_lengths(line: &mut FlexLine, available_main_size: f32) {
    let initial_main_size: f32 = line.items.iter().map(|i| i.main_size).sum();
    let remaining_space = available_main_size - initial_main_size;

    if remaining_space > 0.0 {
        // Grow items
        let total_grow: f32 = line.items.iter().map(|i| i.style.flex_grow).sum();
        if total_grow > 0.0 {
            for item in &mut line.items {
                let grow_ratio = item.style.flex_grow / total_grow;
                item.main_size += remaining_space * grow_ratio;
            }
        }
    } else if remaining_space < 0.0 {
        // Shrink items
        let total_shrink: f32 = line
            .items
            .iter()
            .map(|i| i.style.flex_shrink * i.flex_basis)
            .sum();
        if total_shrink > 0.0 {
            for item in &mut line.items {
                let shrink_ratio = (item.style.flex_shrink * item.flex_basis) / total_shrink;
                item.main_size += remaining_space * shrink_ratio; // remaining_space is negative
            }
        }
    }
    line.main_size = line.items.iter().map(|i| i.main_size).sum();
}

/// Calculates the starting offset and spacing for `justify-content`.
fn calculate_main_axis_alignment(
    free_space: f32,
    item_count: usize,
    justify: &JustifyContent,
) -> (f32, f32) {
    if free_space <= 0.0 {
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
                (free_space / 2.0, 0.0) // Center if only one item
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

/// Calculates the offset for an item on the cross axis based on `align-items`/`align-self`.
fn calculate_cross_axis_alignment(
    item: &FlexItem,
    line_cross_size: f32,
    container_align: &AlignItems,
) -> f32 {
    let align = match &item.style.align_self {
        AlignSelf::Auto => container_align,
        AlignSelf::Stretch => &AlignItems::Stretch,
        AlignSelf::FlexStart => &AlignItems::FlexStart,
        AlignSelf::FlexEnd => &AlignItems::FlexEnd,
        AlignSelf::Center => &AlignItems::Center,
        AlignSelf::Baseline => &AlignItems::Baseline, // TODO: Baseline is not implemented, falls back to FlexStart
    };

    match align {
        AlignItems::Stretch => 0.0, // Stretch is handled by setting item.cross_size earlier, if applicable.
        AlignItems::FlexStart | AlignItems::Baseline => 0.0,
        AlignItems::FlexEnd => line_cross_size - item.cross_size,
        AlignItems::Center => (line_cross_size - item.cross_size) / 2.0,
    }
}