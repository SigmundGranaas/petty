// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/table.rs
use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;

// --- Main Table Node ---

#[derive(Debug, Clone)]
pub struct TableNode {
    header_rows: Vec<TableRowNode>,
    body_rows: Vec<TableRowNode>,
    style: Arc<ComputedStyle>,
    columns: Vec<TableColumnDefinition>,
    calculated_widths: Vec<f32>,
}

impl TableNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let (columns, header, body) = match node {
            IRNode::Table { columns, header, body, .. } => (columns, header, body),
            _ => panic!("TableNode must be created from IRNode::Table"),
        };

        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);

        let header_rows = header
            .as_ref()
            .map(|h| h.rows.iter().map(|r| TableRowNode::new(r, &style, engine)).collect())
            .unwrap_or_default();

        let body_rows = body.rows.iter().map(|r| TableRowNode::new(r, &style, engine)).collect();

        Self {
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

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        self.calculated_widths =
            calculate_column_widths(engine, &self.columns, &self.header_rows, &self.body_rows, available_width);
        for row in self.header_rows.iter_mut().chain(self.body_rows.iter_mut()) {
            row.measure(engine, &self.calculated_widths);
        }
    }

    fn measure_content_height(&mut self, _engine: &LayoutEngine, _available_width: f32) -> f32 {
        self.header_rows.iter().map(|r| r.height).sum::<f32>()
            + self.body_rows.iter().map(|r| r.height).sum::<f32>()
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        let num_rows = self.header_rows.len() + self.body_rows.len();
        let num_cols = self.columns.len();
        let mut occupancy = vec![vec![false; num_cols]; num_rows];
        let mut row_idx_offset = 0;

        for (i, row) in self.header_rows.iter_mut().enumerate() {
            if row.height > ctx.bounds.height {
                return Err(LayoutError::ElementTooLarge(row.height, ctx.bounds.height));
            }
            if row.height > ctx.available_height() && !ctx.is_empty() {
                return Ok(LayoutResult::Partial(Box::new(self.clone())));
            }
            if let Err(e) = row.layout(ctx, &mut occupancy, i, self.style.border_spacing) {
                log::warn!("Skipping table header row that failed to lay out: {}", e);
            }
        }
        row_idx_offset += self.header_rows.len();

        for (i, row) in self.body_rows.iter_mut().enumerate() {
            if row.height > ctx.bounds.height {
                return Err(LayoutError::ElementTooLarge(row.height, ctx.bounds.height));
            }
            if row.height > ctx.available_height() && !ctx.is_empty() {
                let remaining_body_rows = self.body_rows.drain(i..).collect();
                let remainder = TableNode {
                    header_rows: self.header_rows.clone(),
                    body_rows: remaining_body_rows,
                    style: self.style.clone(),
                    columns: self.columns.clone(),
                    calculated_widths: self.calculated_widths.clone(),
                };
                return Ok(LayoutResult::Partial(Box::new(remainder)));
            }
            if let Err(e) = row.layout(ctx, &mut occupancy, row_idx_offset + i, self.style.border_spacing) {
                log::warn!("Skipping table row that failed to lay out: {}", e);
            }
        }
        Ok(LayoutResult::Full)
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

    fn measure(&mut self, engine: &LayoutEngine, col_widths: &[f32]) {
        self.col_widths = col_widths.to_vec();
        let mut max_height: f32 = 0.0;
        for (i, cell) in self.cells.iter_mut().enumerate() {
            let cell_width = col_widths.get(i).copied().unwrap_or(0.0);
            cell.measure(engine, cell_width);
            if cell.rowspan == 1 {
                max_height = max_height.max(cell.height);
            }
        }
        self.height = max_height;
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutContext,
        occupancy: &mut [Vec<bool>],
        row_idx: usize,
        border_spacing: f32,
    ) -> Result<LayoutResult, LayoutError> {
        let mut current_x = 0.0;
        let mut col_idx = 0;
        for cell in self.cells.iter_mut() {
            // Find the next available column, skipping over rowspan placeholders
            while col_idx < self.col_widths.len() && occupancy[row_idx][col_idx] {
                current_x += self.col_widths[col_idx] + border_spacing;
                col_idx += 1;
            }
            if col_idx >= self.col_widths.len() {
                break; // No more space in this row
            }

            let cell_col_idx = col_idx;
            let cell_width = (0..cell.colspan)
                .map(|i| self.col_widths.get(cell_col_idx + i).copied().unwrap_or(0.0))
                .sum::<f32>() + (cell.colspan.saturating_sub(1) as f32 * border_spacing);

            let cell_bounds = geom::Rect {
                x: ctx.bounds.x + current_x,
                y: ctx.bounds.y + ctx.cursor.1,
                width: cell_width,
                height: self.height, // Rowspan cells will overflow this, which is okay for now
            };
            let mut cell_ctx = LayoutContext {
                engine: ctx.engine,
                bounds: cell_bounds,
                cursor: (0.0, 0.0),
                elements: ctx.elements,
                last_v_margin: 0.0,
            };
            cell.layout(&mut cell_ctx)?;

            // Update occupancy grid for future rows
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
        ctx.advance_cursor(self.height + border_spacing);
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
            content: BlockNode::new_from_children(children, cell_style),
            height: 0.0,
            preferred_width: 0.0,
            colspan: cell.colspan.max(1),
            rowspan: cell.rowspan.max(1),
        }
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        self.height = self.content.measure_content_height(engine, available_width);
        // Measure preferred width by giving it "infinite" space
        self.preferred_width = self.content.measure_content_height(engine, f32::MAX);
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        self.content.layout(ctx)
    }
}

// --- Helper Functions ---

fn calculate_column_widths(
    engine: &LayoutEngine,
    columns: &[TableColumnDefinition],
    header_rows: &[TableRowNode],
    body_rows: &[TableRowNode],
    table_width: f32,
) -> Vec<f32> {
    let mut widths = vec![0.0; columns.len()];
    let mut auto_indices = Vec::new();
    let mut remaining_width = table_width;

    // 1. Satisfy fixed and percentage widths
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
                Dimension::Auto => auto_indices.push(i),
            }
        } else {
            auto_indices.push(i);
        }
    }

    if auto_indices.is_empty() {
        return widths;
    }

    // 2. Determine preferred widths for auto columns
    let mut preferred_widths = vec![0.0; columns.len()];
    for row in header_rows.iter().chain(body_rows.iter()) {
        for (i, cell) in row.cells.iter().enumerate() {
            if auto_indices.contains(&i) {
                let mut cell_content = cell.content.clone();
                // A simple heuristic: measure with a very large width to find natural size.
                let preferred = cell_content.measure_content_height(engine, 1_000_000.0);
                let new_preferred: f32 = preferred / cell.colspan as f32;
                preferred_widths[i] = (preferred_widths[i] as f32).max(new_preferred);
            }
        }
    }

    // 3. Distribute remaining width based on preferred widths
    let total_preferred: f32 = auto_indices.iter().map(|&i| preferred_widths[i]).sum();
    if total_preferred > 0.0 && total_preferred < remaining_width {
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