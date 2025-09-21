// src/layout/table.rs

//! Layout logic for tables, rows, and cells.

use super::flex::{layout_subtree, measure_subtree_height};
use super::style::ComputedStyle;
use super::{IRNode, LayoutEngine, PositionedElement, WorkItem};
use crate::idf::{TableBody, TableColumnDefinition, TableHeader, TableRow};
use crate::stylesheet::{Dimension, ElementStyle};
use std::sync::Arc;

/// Lays out a single table row and its cells.
fn layout_table_row(
    engine: &LayoutEngine,
    row: &mut TableRow,
    parent_style: &Arc<ComputedStyle>,
    widths: &[f32],
    start_y: f32,
) -> (Vec<PositionedElement>, f32) {
    // PERF: Pre-allocate vectors with known sizes.
    let mut all_row_elements = Vec::with_capacity(row.cells.len() * 4); // Guess: 4 elements per cell
    let mut cell_layouts = Vec::with_capacity(row.cells.len());
    let mut max_cell_height: f32 = 0.0;
    let mut current_x = 0.0;

    for (i, cell) in row.cells.iter_mut().enumerate() {
        let cell_width = *widths.get(i).unwrap_or(&0.0);
        let cell_style = engine.compute_style(
            cell.style_name.as_deref(),
            cell.style_override.as_ref(),
            parent_style,
        );

        // Create a temporary root node for the cell's children to lay them out.
        let mut cell_root = IRNode::Root(std::mem::take(&mut cell.children));
        let (cell_elements, cell_height) =
            layout_subtree(engine, &mut cell_root, &cell_style, cell_width);

        // Restore children
        if let IRNode::Root(children) = cell_root {
            cell.children = children;
        }

        max_cell_height = max_cell_height.max(cell_height);
        cell_layouts.push((cell_elements, current_x));
        current_x += cell_width;
    }

    for (mut cell_elements, x_pos) in cell_layouts {
        for el in &mut cell_elements {
            el.x += x_pos;
            el.y += start_y;
        }
        all_row_elements.extend(cell_elements);
    }
    (all_row_elements, max_cell_height)
}

/// Helper to lay out a slice of rows and return their elements and total height.
fn layout_rows(
    engine: &LayoutEngine,
    rows: &mut [TableRow],
    parent_style: &Arc<ComputedStyle>,
    widths: &[f32],
    start_y: f32,
) -> (Vec<PositionedElement>, f32) {
    let mut all_row_elements = Vec::with_capacity(rows.len() * widths.len() * 4); // Guess: rows * cells * elements_per_cell
    let mut current_y = start_y;
    for row in rows {
        let (row_els, row_height) = layout_table_row(engine, row, parent_style, widths, current_y);
        all_row_elements.extend(row_els);
        current_y += row_height;
    }
    (all_row_elements, current_y - start_y)
}

/// Lays out a full table, handling pagination by returning remaining rows.
pub fn layout_table(
    engine: &LayoutEngine,
    header: Option<&mut TableHeader>,
    body: &mut TableBody,
    style: &Arc<ComputedStyle>,
    widths: &[f32],
    available_height: f32,
) -> (Vec<PositionedElement>, f32, Option<Box<TableBody>>) {
    // PERF: Pre-allocate with a reasonable guess.
    let mut elements = Vec::with_capacity(body.rows.len() * widths.len());
    let mut total_height = 0.0;

    // 1. Layout header
    if let Some(h) = header {
        let (header_elements, header_height) = layout_rows(engine, &mut h.rows, style, widths, 0.0);

        if header_height > available_height {
            // Header is too big for the available space. Push the entire body to the next page.
            let remainder = Box::new(std::mem::take(body));
            return (vec![], 0.0, Some(remainder));
        }
        elements.extend(header_elements);
        total_height += header_height;
    }

    // 2. Perform a cheap measurement pass first.
    let mut all_rows = std::mem::take(&mut body.rows);
    let mut split_idx = 0;
    let mut current_measured_height = total_height;

    for (i, row) in all_rows.iter_mut().enumerate() {
        // Use a cheap, non-allocating measurement function.
        let row_height = measure_table_row(engine, row, style, widths);

        if current_measured_height + row_height > available_height && i > 0 {
            split_idx = i;
            break;
        }
        current_measured_height += row_height;
        split_idx = i + 1;
    }

    // 3. Separate the processed rows from the remainder.
    let remaining_rows_vec = all_rows.split_off(split_idx);
    let mut rows_for_this_page = all_rows;

    // 4. Perform the expensive layout ONLY on the rows that fit.
    if !rows_for_this_page.is_empty() {
        let (row_elements, rows_height) = layout_rows(engine, &mut rows_for_this_page, style, widths, total_height);
        elements.extend(row_elements);
        total_height += rows_height;
    }


    // 5. Update the body node with the rows that were laid out on this page.
    body.rows = rows_for_this_page;

    // 6. Package the remaining rows into a new TableBody to be returned.
    let remainder = if remaining_rows_vec.is_empty() {
        None
    } else {
        Some(Box::new(TableBody {
            rows: remaining_rows_vec,
        }))
    };

    (elements, total_height, remainder)
}

/// A cheap, measurement-only version of layout_table_row.
fn measure_table_row(
    engine: &LayoutEngine,
    row: &mut TableRow,
    parent_style: &Arc<ComputedStyle>,
    widths: &[f32],
) -> f32 {
    let mut max_cell_height: f32 = 0.0;

    for (i, cell) in row.cells.iter_mut().enumerate() {
        let cell_width = *widths.get(i).unwrap_or(&0.0);
        let cell_style = engine.compute_style(
            cell.style_name.as_deref(),
            cell.style_override.as_ref(),
            parent_style,
        );

        // Create a temporary root node to measure the cell's children without cloning them.
        let mut cell_root = IRNode::Root(std::mem::take(&mut cell.children));
        let cell_height = measure_subtree_height(engine, &mut cell_root, &cell_style, cell_width);

        // Restore the children to the cell.
        if let IRNode::Root(children) = cell_root {
            cell.children = children;
        }
        max_cell_height = max_cell_height.max(cell_height);
    }
    max_cell_height
}


/// Lays out a full table node, handling pagination.
/// This function is the public API for laying out a table, called from the page layout dispatcher.
pub fn layout_table_node(
    engine: &LayoutEngine,
    style_name: &Option<String>,
    style_override: &Option<ElementStyle>,
    columns: &[TableColumnDefinition],
    header: &mut Option<Box<TableHeader>>,
    body: &mut TableBody,
    calculated_widths: &[f32],
    style: &Arc<ComputedStyle>,
    available_height: f32,
) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
    let max_height_for_table =
        available_height - style.padding.top - style.padding.bottom - style.margin.bottom;

    // Clone the header *before* layout, so we have a clean copy for the next page.
    let original_header = header.clone();

    let (els, height, remaining_body) = layout_table(
        engine,
        header.as_deref_mut(),
        body,
        style,
        calculated_widths,
        max_height_for_table.max(0.0),
    );

    let pending_work = if let Some(rem_body) = remaining_body {
        Some(WorkItem::Node(IRNode::Table {
            style_name: style_name.clone(),
            style_override: style_override.clone(),
            columns: columns.to_vec(),
            calculated_widths: calculated_widths.to_vec(),
            header: original_header, // Use the clean, cloned header
            body: rem_body,
        }))
    } else {
        None
    };

    (els, height, pending_work)
}

/// Calculates final column widths based on definitions and available space.
pub fn calculate_column_widths(
    columns: &[crate::idf::TableColumnDefinition],
    available_width: f32,
) -> Vec<f32> {
    if columns.is_empty() {
        return Vec::new();
    }
    let mut widths = vec![0.0; columns.len()];
    let mut remaining_width = available_width;
    let mut percent_total = 0.0;
    let mut auto_indices = Vec::with_capacity(columns.len());
    for (i, col) in columns.iter().enumerate() {
        if let Some(dim) = &col.width {
            match dim {
                Dimension::Pt(w) => {
                    widths[i] = *w;
                    remaining_width -= *w;
                }
                Dimension::Percent(p) => percent_total += p,
                Dimension::Auto => auto_indices.push(i),
                Dimension::Px(_) => { /* Px is treated as Pt for now */ }
            }
        } else {
            auto_indices.push(i);
        }
    }
    if percent_total > 0.0 {
        let width_for_percent = remaining_width.max(0.0);
        for (i, col) in columns.iter().enumerate() {
            if let Some(Dimension::Percent(p)) = &col.width {
                let new_width = (p / 100.0) * width_for_percent;
                widths[i] = new_width;
                remaining_width -= new_width;
            }
        }
    }
    if !auto_indices.is_empty() {
        let width_per_auto = remaining_width.max(0.0) / auto_indices.len() as f32;
        for i in auto_indices {
            widths[i] = width_per_auto;
        }
    }
    widths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::TableColumnDefinition;

    #[test]
    fn test_calculate_column_widths_all_auto() {
        let columns = vec![
            TableColumnDefinition {
                width: Some(Dimension::Auto),
                ..Default::default()
            },
            TableColumnDefinition {
                width: Some(Dimension::Auto),
                ..Default::default()
            },
            TableColumnDefinition {
                width: Some(Dimension::Auto),
                ..Default::default()
            },
        ];
        let widths = calculate_column_widths(&columns, 300.0);
        assert_eq!(widths, vec![100.0, 100.0, 100.0]);
    }

    #[test]
    fn test_calculate_column_widths_mixed() {
        let columns = vec![
            TableColumnDefinition {
                width: Some(Dimension::Pt(50.0)),
                ..Default::default()
            },
            TableColumnDefinition {
                width: Some(Dimension::Percent(50.0)),
                ..Default::default()
            },
            TableColumnDefinition {
                width: Some(Dimension::Auto),
                ..Default::default()
            },
        ];
        // Available: 300. Pt takes 50. Remaining for %/auto: 250.
        // % takes 50% of 250 = 125. Remaining: 125.
        // Auto takes the rest: 125.
        let widths = calculate_column_widths(&columns, 300.0);
        assert_eq!(widths, vec![50.0, 125.0, 125.0]);
    }

    #[test]
    fn test_calculate_column_widths_no_auto() {
        let columns = vec![
            TableColumnDefinition {
                width: Some(Dimension::Pt(100.0)),
                ..Default::default()
            },
            TableColumnDefinition {
                width: Some(Dimension::Percent(100.0)),
                ..Default::default()
            },
        ];
        // Available: 500. Pt takes 100. Remaining for %: 400.
        // % takes 100% of 400 = 400.
        let widths = calculate_column_widths(&columns, 500.0);
        assert_eq!(widths, vec![100.0, 400.0]);
    }

    // Add default impl for test struct
    impl Default for TableColumnDefinition {
        fn default() -> Self {
            Self {
                width: None,
                style: None,
                header_style: None,
            }
        }
    }
}