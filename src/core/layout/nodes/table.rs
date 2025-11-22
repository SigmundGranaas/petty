use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{AnchorLocation, LayoutBuffer, LayoutEnvironment, LayoutNode, LayoutResult};
use crate::core::layout::nodes::block::{draw_background_and_borders, BlockNode};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;
use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
// --- Main Table Node ---

#[derive(Debug, Clone)]
pub struct TableNode {
    id: Option<String>,
    header_rows: Vec<TableRowNode>,
    body_rows: Vec<TableRowNode>,
    style: Arc<ComputedStyle>,
    columns: Vec<TableColumnDefinition>,
    calculated_widths: Vec<f32>,
}

impl TableNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let (meta, columns, header, body) = match node {
            IRNode::Table { meta, columns, header, body, .. } => (meta, columns, header, body),
            _ => panic!("TableNode must be created from IRNode::Table"),
        };

        let style = engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), &parent_style);

        let header_rows = header
            .as_ref()
            .map(|h| h.rows.iter().map(|r| TableRowNode::new(r, &style, engine)).collect())
            .unwrap_or_default();

        let body_rows = body.rows.iter().map(|r| TableRowNode::new(r, &style, engine)).collect();

        Self {
            id: meta.id.clone(),
            header_rows,
            body_rows,
            style,
            columns: columns.clone(),
            calculated_widths: Vec::new(),
        }
    }
}

impl LayoutNode for TableNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);
        let padding_x = self.style.padding.left + self.style.padding.right;

        let available_width = if constraints.has_bounded_width() {
            (constraints.max_width - padding_x - border_left_width - border_right_width).max(0.0)
        } else {
            f32::INFINITY
        };

        self.calculated_widths = calculate_column_widths(
            env,
            &self.columns,
            &mut self.header_rows,
            &mut self.body_rows,
            available_width,
        );

        let mut total_height = 0.0;
        for row in self.header_rows.iter_mut().chain(self.body_rows.iter_mut()) {
            row.measure(env, &self.calculated_widths);
            total_height += row.height;
        }

        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let padding_y = self.style.padding.top + self.style.padding.bottom;
        let total_height = self.style.margin.top
            + border_top_width
            + padding_y
            + total_height
            + self.style.padding.bottom
            + border_bottom_width
            + self.style.margin.bottom;

        let width = if constraints.has_bounded_width() { constraints.max_width } else {
            self.calculated_widths.iter().sum::<f32>() + padding_x + border_left_width + border_right_width
        };

        Size::new(width, total_height)
    }

    fn layout(&mut self, env: &LayoutEnvironment, buf: &mut LayoutBuffer) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: env.local_page_index,
                y_pos: buf.cursor.1 + buf.bounds.y,
            };
            buf.defined_anchors.insert(id.clone(), location);
        }

        // --- Box Model Setup ---
        let margin_to_add = self.style.margin.top.max(buf.last_v_margin);
        if !buf.is_empty() && margin_to_add > buf.available_height() {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }
        buf.advance_cursor(margin_to_add);
        buf.last_v_margin = 0.0;

        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);

        let block_start_y_in_ctx = buf.cursor.1;
        buf.advance_cursor(border_top_width + self.style.padding.top);
        let content_start_y_in_ctx = buf.cursor.1;

        // --- Table Layout Logic ---
        let child_bounds = geom::Rect {
            x: buf.bounds.x + border_left_width + self.style.padding.left,
            y: buf.bounds.y + content_start_y_in_ctx,
            width: buf.bounds.width
                - self.style.padding.left
                - self.style.padding.right
                - border_left_width
                - border_right_width,
            height: buf.available_height(),
        };
        let mut child_buf = LayoutBuffer {
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: &mut *buf.elements,
            last_v_margin: 0.0,
            defined_anchors: &mut *buf.defined_anchors,
            index_entries: &mut *buf.index_entries,
        };

        let num_cols = self.columns.len();
        let num_rows = self.header_rows.len() + self.body_rows.len();
        let mut occupancy = vec![vec![false; num_cols]; num_rows];
        let mut row_idx_offset = 0;
        let mut content_height_on_this_page = 0.0;

        for (i, row) in self.header_rows.iter_mut().enumerate() {
            if row.height > child_buf.bounds.height {
                return Err(LayoutError::ElementTooLarge(row.height, child_buf.bounds.height));
            }
            if row.height > child_buf.available_height() && !child_buf.is_empty() {
                // This case should be rare (header taller than page), but we handle it by breaking.
                let remainder = self.body_rows.drain(..).collect();
                return Ok(LayoutResult::Partial(Box::new(create_remainder_table(self, remainder))));
            }
            if let Err(e) = row.layout(env, &mut child_buf, &mut occupancy, i, self.style.border_spacing) {
                log::warn!("Skipping table header row that failed to lay out: {}", e);
            }
        }
        content_height_on_this_page += child_buf.cursor.1;
        row_idx_offset += self.header_rows.len();

        for (i, row) in self.body_rows.iter_mut().enumerate() {
            if row.height > child_buf.bounds.height {
                return Err(LayoutError::ElementTooLarge(row.height, child_buf.bounds.height));
            }
            if row.height > child_buf.available_height() && !child_buf.is_empty() {
                draw_background_and_borders(
                    child_buf.elements,
                    buf.bounds,
                    &self.style,
                    block_start_y_in_ctx,
                    content_height_on_this_page
                );

                buf.cursor.1 = content_start_y_in_ctx + content_height_on_this_page + self.style.padding.bottom + border_bottom_width;

                let remaining_body_rows = self.body_rows.drain(i..).collect();

                let mut remainder = create_remainder_table(self, remaining_body_rows);
                // Remainder also constrained by page width
                remainder.measure(env, BoxConstraints::tight_width(buf.bounds.width));
                return Ok(LayoutResult::Partial(Box::new(remainder)));
            }
            if let Err(e) = row.layout(env, &mut child_buf, &mut occupancy, row_idx_offset + i, self.style.border_spacing) {
                log::warn!("Skipping table row that failed to lay out: {}", e);
            }
        }

        content_height_on_this_page = child_buf.cursor.1;
        draw_background_and_borders(
            child_buf.elements,
            buf.bounds,
            &self.style,
            block_start_y_in_ctx,
            content_height_on_this_page
        );
        buf.cursor.1 = content_start_y_in_ctx + content_height_on_this_page + self.style.padding.bottom + border_bottom_width;
        buf.last_v_margin = self.style.margin.bottom;

        Ok(LayoutResult::Full)
    }
}

fn create_remainder_table(original_table: &TableNode, remaining_body_rows: Vec<TableRowNode>) -> TableNode {
    TableNode {
        id: original_table.id.clone(),
        header_rows: original_table.header_rows.clone(),
        body_rows: remaining_body_rows,
        style: original_table.style.clone(),
        columns: original_table.columns.clone(),
        calculated_widths: original_table.calculated_widths.clone(),
    }
}

// --- Table Row Node ---

#[derive(Debug, Clone)]
pub struct TableRowNode {
    cells: Vec<TableCellNode>,
    height: f32,
    col_widths: Vec<f32>,
}

impl TableRowNode {
    fn new(row: &TableRow, style: &Arc<ComputedStyle>, engine: &LayoutEngine) -> Self {
        let cells = row.cells.iter().map(|c| TableCellNode::new(c, style, engine)).collect();
        Self {
            cells,
            height: 0.0,
            col_widths: vec![],
        }
    }

    fn measure(&mut self, env: &LayoutEnvironment, col_widths: &[f32]) {
        self.col_widths = col_widths.to_vec();
        let mut max_height: f32 = 0.0;
        let mut col_cursor = 0;
        for cell in self.cells.iter_mut() {
            if col_cursor >= col_widths.len() { break; }
            let end_col = (col_cursor + cell.colspan).min(col_widths.len());
            let cell_width: f32 = col_widths[col_cursor..end_col].iter().sum();
            cell.measure(env, cell_width);
            if cell.rowspan == 1 {
                max_height = max_height.max(cell.height);
            }
            col_cursor += cell.colspan;
        }
        self.height = max_height;
    }

    fn layout(
        &mut self,
        env: &LayoutEnvironment,
        buf: &mut LayoutBuffer,
        occupancy: &mut [Vec<bool>],
        row_idx: usize,
        border_spacing: f32,
    ) -> Result<LayoutResult, LayoutError> {
        let mut current_x = 0.0;
        let mut col_idx = 0;
        for cell in self.cells.iter_mut() {
            while col_idx < self.col_widths.len() && occupancy.get(row_idx).and_then(|r| r.get(col_idx)).map_or(false, |&o| o) {
                current_x += self.col_widths[col_idx] + border_spacing;
                col_idx += 1;
            }
            if col_idx >= self.col_widths.len() {
                break;
            }

            let cell_col_idx = col_idx;
            let cell_width = (0..cell.colspan)
                .map(|i| self.col_widths.get(cell_col_idx + i).copied().unwrap_or(0.0))
                .sum::<f32>() + (cell.colspan.saturating_sub(1) as f32 * border_spacing);

            let cell_bounds = geom::Rect {
                x: buf.bounds.x + current_x,
                y: buf.bounds.y + buf.cursor.1,
                width: cell_width,
                height: self.height,
            };
            let mut cell_buf = LayoutBuffer {
                bounds: cell_bounds,
                cursor: (0.0, 0.0),
                elements: &mut *buf.elements,
                last_v_margin: 0.0,
                defined_anchors: &mut *buf.defined_anchors,
                index_entries: &mut *buf.index_entries,
            };
            cell.layout(env, &mut cell_buf)?;

            for r in 0..cell.rowspan {
                for c in 0..cell.colspan {
                    if let Some(occ_row) = occupancy.get_mut(row_idx + r) {
                        if let Some(occ_cell) = occ_row.get_mut(cell_col_idx + c) {
                            *occ_cell = true;
                        }
                    }
                }
            }
            current_x += cell_width + border_spacing;
            col_idx += cell.colspan;
        }
        buf.advance_cursor(self.height + border_spacing);
        Ok(LayoutResult::Full)
    }
}

// --- Table Cell Node ---

#[derive(Debug, Clone)]
struct TableCellNode {
    content: BlockNode,
    height: f32,
    preferred_width: f32,
    colspan: usize,
    rowspan: usize,
}

impl TableCellNode {
    fn new(cell: &crate::core::idf::TableCell, style: &Arc<ComputedStyle>, engine: &LayoutEngine) -> Self {
        let cell_style = engine.compute_style(&cell.style_sets, cell.style_override.as_ref(), style);
        let children = cell
            .children
            .iter()
            .map(|c| engine.build_layout_node_tree(c, cell_style.clone()))
            .collect();
        Self {
            content: BlockNode::new_from_children(None, children, cell_style),
            height: 0.0,
            preferred_width: 0.0,
            colspan: cell.col_span.max(1),
            rowspan: cell.row_span.max(1),
        }
    }

    fn measure(&mut self, env: &LayoutEnvironment, available_width: f32) {
        // Measure height with fixed width
        let constraint_fixed = BoxConstraints::tight_width(available_width);
        self.height = self.content.measure(env, constraint_fixed).height;

        // Measure preferred width by giving it "infinite" space
        let constraint_infinite = BoxConstraints {
            min_width: 0.0, max_width: f32::INFINITY,
            min_height: 0.0, max_height: f32::INFINITY
        };
        self.preferred_width = self.content.measure(env, constraint_infinite).width;
    }

    fn layout(&mut self, env: &LayoutEnvironment, buf: &mut LayoutBuffer) -> Result<LayoutResult, LayoutError> {
        self.content.layout(env, buf)
    }
}

// --- Helper Functions ---

fn calculate_column_widths(
    env: &LayoutEnvironment,
    columns: &[TableColumnDefinition],
    header_rows: &mut [TableRowNode],
    body_rows: &mut [TableRowNode],
    table_width: f32,
) -> Vec<f32> {
    let mut widths = vec![0.0; columns.len()];
    let mut auto_indices = Vec::new();
    let mut remaining_width = table_width;

    // If table width is infinite (unbounded), we just size everything by preferred/intrinsic width?
    // Or treat percents as 0.
    let table_width_is_finite = table_width.is_finite();

    // 1. Satisfy fixed and percentage widths
    for (i, col) in columns.iter().enumerate() {
        if let Some(dim) = &col.width {
            match dim {
                Dimension::Pt(w) => {
                    widths[i] = *w;
                    remaining_width -= *w;
                }
                Dimension::Percent(p) => {
                    if table_width_is_finite {
                        widths[i] = (p / 100.0) * table_width;
                        remaining_width -= widths[i];
                    } else {
                        auto_indices.push(i); // Treat as auto if unbounded
                    }
                }
                Dimension::Auto => auto_indices.push(i),
            }
        } else {
            auto_indices.push(i);
        }
    }
    remaining_width = remaining_width.max(0.0);

    if auto_indices.is_empty() {
        return widths;
    }

    // 2. Determine preferred widths for auto columns
    let mut preferred_widths: Vec<f32> = vec![0.0; columns.len()];

    // Helper to measure cell preferred width.
    // Since we are mutably borrowing rows, we need to be careful.
    // But actually, table cells store preferred width internally after we call measure on them?
    // No, we calculate widths BEFORE calling measure on rows with those widths.
    // So we must probe the cells now.

    let infinite_constraint = BoxConstraints {
        min_width: 0.0, max_width: f32::INFINITY,
        min_height: 0.0, max_height: f32::INFINITY
    };

    for row in header_rows.iter_mut().chain(body_rows.iter_mut()) {
        let mut col_cursor = 0;
        for cell in &mut row.cells {
            if col_cursor >= columns.len() { break; }
            if auto_indices.contains(&col_cursor) { // Simplified: only considers first col of a span
                let preferred = cell.content.measure(env, infinite_constraint).width;
                if cell.colspan == 1 {
                    preferred_widths[col_cursor] = preferred_widths[col_cursor].max(preferred);
                }
            }
            col_cursor += cell.colspan;
        }
    }

    // 3. Distribute remaining width based on preferred widths
    let total_preferred: f32 = auto_indices.iter().map(|&i| preferred_widths[i]).sum();

    if !table_width_is_finite {
        // Just use preferred widths
        for &i in &auto_indices {
            widths[i] = preferred_widths[i];
        }
        return widths;
    }

    if total_preferred > 0.0 && remaining_width > total_preferred {
        // Distribute extra space proportionally
        let extra_space = remaining_width - total_preferred;
        for &i in &auto_indices {
            widths[i] = preferred_widths[i] + extra_space * (preferred_widths[i] / total_preferred);
        }
    } else if total_preferred > 0.0 {
        // Shrink proportionally
        let shrink_factor = remaining_width / total_preferred;
        for &i in &auto_indices {
            widths[i] = preferred_widths[i] * shrink_factor;
        }
    } else {
        // No preferred widths, distribute equally
        let width_per_auto = remaining_width / auto_indices.len() as f32;
        for i in auto_indices {
            widths[i] = width_per_auto;
        }
    }
    widths
}