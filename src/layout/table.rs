// src/layout/table.rs

//! Layout logic for tables, rows, and cells.

use super::flex::layout_subtree; // Re-use the subtree layout for cells
use super::style::{ ComputedStyle};
use super::{IRNode, LayoutEngine, PositionedElement};
use crate::idf::{TableBody, TableHeader, TableRow};
use crate::stylesheet::Dimension;

/// Lays out a full table, including header and body.
pub fn layout_table(
    engine: &LayoutEngine,
    header: Option<&mut TableHeader>,
    body: &mut TableBody,
    style: &ComputedStyle,
    widths: &[f32],
) -> (Vec<PositionedElement>, f32, Option<super::WorkItem>) {
    let mut elements = Vec::new();
    let mut total_height = 0.0;

    if let Some(h) = header {
        for row in &mut h.rows {
            let (row_els, row_height) = layout_table_row(engine, row, style, widths, total_height);
            elements.extend(row_els);
            total_height += row_height;
        }
    }
    for row in &mut body.rows {
        let (row_els, row_height) = layout_table_row(engine, row, style, widths, total_height);
        elements.extend(row_els);
        total_height += row_height;
    }

    (elements, total_height, None)
}

/// Lays out a single table row and its cells.
fn layout_table_row(
    engine: &LayoutEngine,
    row: &mut TableRow,
    parent_style: &ComputedStyle,
    widths: &[f32],
    start_y: f32,
) -> (Vec<PositionedElement>, f32) {
    let mut all_row_elements = Vec::new();
    let mut max_cell_height: f32 = 0.0;
    let mut cell_layouts = Vec::new();
    let mut current_x = 0.0;

    for (i, cell) in row.cells.iter_mut().enumerate() {
        let cell_width = *widths.get(i).unwrap_or(&0.0);
        let cell_style = engine.compute_style(cell.style_override.as_deref(), parent_style);

        // Create a temporary root node for the cell's children to lay them out.
        let mut cell_root = IRNode::Root(std::mem::take(&mut cell.children));
        let (cell_elements, cell_height) = layout_subtree(engine, &mut cell_root, &cell_style, cell_width);

        // Restore children
        if let IRNode::Root(children) = cell_root { cell.children = children; }

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


/// Calculates final column widths based on definitions and available space.
pub fn calculate_column_widths(
    columns: &[crate::idf::TableColumnDefinition],
    available_width: f32,
) -> Vec<f32> {
    let mut widths = vec![0.0; columns.len()];
    let mut remaining_width = available_width;
    let mut percent_total = 0.0;
    let mut auto_indices = Vec::new();
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
            TableColumnDefinition { width: Some(Dimension::Auto), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Auto), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Auto), ..Default::default() },
        ];
        let widths = calculate_column_widths(&columns, 300.0);
        assert_eq!(widths, vec![100.0, 100.0, 100.0]);
    }

    #[test]
    fn test_calculate_column_widths_mixed() {
        let columns = vec![
            TableColumnDefinition { width: Some(Dimension::Pt(50.0)), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Percent(50.0)), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Auto), ..Default::default() },
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
            TableColumnDefinition { width: Some(Dimension::Pt(100.0)), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Percent(100.0)), ..Default::default() },
        ];
        // Available: 500. Pt takes 100. Remaining for %: 400.
        // % takes 100% of 400 = 400.
        let widths = calculate_column_widths(&columns, 500.0);
        assert_eq!(widths, vec![100.0, 400.0]);
    }

    // Add default impl for test struct
    impl Default for TableColumnDefinition {
        fn default() -> Self {
            Self { width: None, style: None, header_style: None }
        }
    }
}