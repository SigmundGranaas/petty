
use super::style::ComputedStyle;
use super::{IRNode, LayoutBox, LayoutContent, LayoutEngine, Rect};
use crate::core::idf::{TableColumnDefinition, TableRow};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

/// Calculates the absolute widths of table columns based on their definitions.
pub(super) fn calculate_column_widths(
    columns: &[TableColumnDefinition],
    table_width: f32,
) -> Vec<f32> {
    let mut widths = vec![0.0; columns.len()];
    let mut auto_indices = vec![];
    let mut remaining_width = table_width;

    // First pass for fixed and percentage widths
    for (i, col) in columns.iter().enumerate() {
        if let Some(dim) = &col.width {
            match dim {
                Dimension::Pt(w) => {
                    widths[i] = *w;
                    remaining_width -= *w;
                }
                Dimension::Percent(p) => {
                    widths[i] = (p / 100.0) * table_width;
                    remaining_width -= widths[i];
                }
                _ => auto_indices.push(i),
            }
        } else {
            auto_indices.push(i);
        }
    }

    // Second pass for auto widths
    if !auto_indices.is_empty() && remaining_width > 0.0 {
        let width_per_auto = remaining_width / auto_indices.len() as f32;
        for i in auto_indices {
            widths[i] = width_per_auto;
        }
    }

    widths
}

/// Lays out a full table, handling pagination by returning remaining rows.
pub fn layout_table(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: Arc<ComputedStyle>,
    _available_size: (f32, f32),
) -> LayoutBox {
    let (header, body, _columns, calculated_widths) = match node {
        IRNode::Table {
            header,
            body,
            columns,
            calculated_widths,
            ..
        } => (header, body, columns, calculated_widths),
        _ => {
            return LayoutBox {
                rect: Default::default(),
                style,
                content: LayoutContent::Children(vec![]),
            }
        }
    };

    // TODO: This is a simplified non-paginating layout. A full implementation would
    // need to use `available_size.1` to detect when a row will not fit on the current
    // page, stop layout, and signal to the pagination engine that the remaining rows
    // need to be processed on the next page.
    let mut child_boxes = vec![];
    let mut current_y = 0.0;

    if let Some(h) = header.as_mut() {
        for row in &mut h.rows {
            let (row_boxes, row_height) =
                layout_table_row(engine, row, &style, calculated_widths, current_y);
            child_boxes.extend(row_boxes);
            current_y += row_height;
        }
    }

    for row in &mut body.rows {
        let (row_boxes, row_height) =
            layout_table_row(engine, row, &style, calculated_widths, current_y);
        child_boxes.extend(row_boxes);
        current_y += row_height;
    }

    LayoutBox {
        rect: Rect {
            height: current_y,
            ..Default::default()
        },
        style,
        content: LayoutContent::Children(child_boxes),
    }
}

/// Lays out a single table row and its cells.
fn layout_table_row(
    engine: &LayoutEngine,
    row: &mut TableRow,
    parent_style: &Arc<ComputedStyle>,
    widths: &[f32],
    start_y: f32,
) -> (Vec<LayoutBox>, f32) {
    let mut cell_boxes = Vec::new();
    let mut max_cell_height: f32 = 0.0;
    let mut current_x = 0.0;

    for (i, cell) in row.cells.iter_mut().enumerate() {
        let cell_width = *widths.get(i).unwrap_or(&0.0);
        let cell_style = engine.compute_style(
            &cell.style_sets,
            cell.style_override.as_ref(),
            parent_style,
        );
        let available_size = (cell_width, f32::INFINITY);

        // Create a temporary root node for the cell's children to lay them out.
        // This is a common pattern to reuse the layout engine for a sub-tree.
        let mut cell_root = IRNode::Root(std::mem::take(&mut cell.children));
        let mut cell_box = engine.build_layout_tree(&mut cell_root, cell_style, available_size);

        // Restore children
        if let IRNode::Root(children) = cell_root {
            cell.children = children;
        }

        cell_box.rect.x = current_x;
        cell_box.rect.y = start_y;

        max_cell_height = max_cell_height.max(cell_box.rect.height);
        cell_boxes.push(cell_box);
        current_x += cell_width;
    }

    // Ensure all cells in the row have the same height.
    for cell_box in &mut cell_boxes {
        cell_box.rect.height = max_cell_height;
    }


    (cell_boxes, max_cell_height)
}