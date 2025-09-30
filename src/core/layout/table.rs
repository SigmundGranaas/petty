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

/// Lays out a full table by creating a structured `LayoutBox` tree of rows and cells.
/// This structure allows the pagination engine to keep rows together.
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

    let mut row_boxes = vec![];
    let mut current_y = 0.0;

    if let Some(h) = header.as_mut() {
        for row in &mut h.rows {
            let mut row_box = layout_table_row(engine, row, &style, calculated_widths);
            row_box.rect.y = current_y;
            current_y += row_box.rect.height;
            row_boxes.push(row_box);
        }
    }

    for row in &mut body.rows {
        let mut row_box = layout_table_row(engine, row, &style, calculated_widths);
        row_box.rect.y = current_y;
        current_y += row_box.rect.height;
        row_boxes.push(row_box);
    }

    LayoutBox {
        rect: Rect {
            height: current_y,
            ..Default::default()
        },
        style,
        content: LayoutContent::Children(row_boxes),
    }
}

/// Lays out a single table row and its cells, returning a single `LayoutBox` for the row.
fn layout_table_row(
    engine: &LayoutEngine,
    row: &mut TableRow,
    parent_style: &Arc<ComputedStyle>,
    widths: &[f32],
) -> LayoutBox {
    let mut cell_boxes = Vec::new();
    let mut max_cell_height: f32 = 0.0;

    // First pass: layout all cells to determine their heights and find the max.
    for (i, cell) in row.cells.iter_mut().enumerate() {
        let cell_width = *widths.get(i).unwrap_or(&0.0);
        let cell_style =
            engine.compute_style(&cell.style_sets, cell.style_override.as_ref(), parent_style);

        // Create a temporary root node for the cell's children to lay them out.
        // This reuses the main layout logic for a sub-tree.
        let mut cell_root = IRNode::Root(std::mem::take(&mut cell.children));
        // The returned box represents the entire cell, including its padding and content.
        let cell_box =
            engine.build_layout_tree(&mut cell_root, cell_style, (cell_width, f32::INFINITY));

        // Restore children
        if let IRNode::Root(children) = cell_root {
            cell.children = children;
        }

        max_cell_height = max_cell_height.max(cell_box.rect.height);
        cell_boxes.push(cell_box);
    }

    // Second pass: position cells horizontally and enforce the uniform row height.
    let mut final_boxes = vec![];
    let mut current_x = 0.0;
    for mut cell_box in cell_boxes {
        cell_box.rect.x = current_x;
        cell_box.rect.y = 0.0; // Position is relative to the row's content box.
        cell_box.rect.height = max_cell_height;
        current_x += cell_box.rect.width;
        final_boxes.push(cell_box);
    }

    // Return a single LayoutBox for the entire row.
    LayoutBox {
        rect: Rect {
            x: 0.0,
            y: 0.0, // Y position is set by the table layout function.
            width: current_x,
            height: max_cell_height,
        },
        style: parent_style.clone(), // A row box doesn't have its own style, so it inherits.
        content: LayoutContent::Children(final_boxes),
    }
}