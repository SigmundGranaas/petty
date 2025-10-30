use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
use crate::core::layout::node::{AnchorLocation, LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::block::{draw_background_and_borders, BlockNode};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;

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

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        let border_left_width = self.style.border_left.as_ref().map_or(0.0, |b| b.width);
        let border_right_width = self.style.border_right.as_ref().map_or(0.0, |b| b.width);
        let child_available_width = available_width
            - self.style.padding.left
            - self.style.padding.right
            - border_left_width
            - border_right_width;

        self.calculated_widths = calculate_column_widths(
            engine,
            &self.columns,
            &self.header_rows,
            &self.body_rows,
            child_available_width,
        );
        for row in self.header_rows.iter_mut().chain(self.body_rows.iter_mut()) {
            row.measure(engine, &self.calculated_widths);
        }
    }

    fn measure_content_height(&mut self, _engine: &LayoutEngine, _available_width: f32) -> f32 {
        let content_height = self.header_rows.iter().map(|r| r.height).sum::<f32>()
            + self.body_rows.iter().map(|r| r.height).sum::<f32>();

        let border_top_width = self.style.border_top.as_ref().map_or(0.0, |b| b.width);
        let border_bottom_width = self.style.border_bottom.as_ref().map_or(0.0, |b| b.width);

        self.style.margin.top
            + border_top_width
            + self.style.padding.top
            + content_height
            + self.style.padding.bottom
            + border_bottom_width
            + self.style.margin.bottom
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

        // --- Table Layout Logic ---
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
        let mut child_ctx = LayoutContext {
            engine: ctx.engine,
            bounds: child_bounds,
            cursor: (0.0, 0.0),
            elements: ctx.elements,
            last_v_margin: 0.0,
            local_page_index: ctx.local_page_index,
            defined_anchors: ctx.defined_anchors,
        };

        let num_cols = self.columns.len();
        let num_rows = self.header_rows.len() + self.body_rows.len();
        let mut occupancy = vec![vec![false; num_cols]; num_rows];
        let mut row_idx_offset = 0;
        let mut content_height_on_this_page = 0.0;

        for (i, row) in self.header_rows.iter_mut().enumerate() {
            if row.height > child_ctx.bounds.height {
                return Err(LayoutError::ElementTooLarge(row.height, child_ctx.bounds.height));
            }
            if row.height > child_ctx.available_height() && !child_ctx.is_empty() {
                // This case should be rare (header taller than page), but we handle it by breaking.
                let remainder = self.body_rows.drain(..).collect();
                return Ok(LayoutResult::Partial(Box::new(create_remainder_table(self, remainder))));
            }
            if let Err(e) = row.layout(&mut child_ctx, &mut occupancy, i, self.style.border_spacing) {
                log::warn!("Skipping table header row that failed to lay out: {}", e);
            }
        }
        content_height_on_this_page += child_ctx.cursor.1;
        row_idx_offset += self.header_rows.len();

        for (i, row) in self.body_rows.iter_mut().enumerate() {
            if row.height > child_ctx.bounds.height {
                return Err(LayoutError::ElementTooLarge(row.height, child_ctx.bounds.height));
            }
            if row.height > child_ctx.available_height() && !child_ctx.is_empty() {
                draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height_on_this_page);
                ctx.cursor.1 = content_start_y_in_ctx + content_height_on_this_page + self.style.padding.bottom + border_bottom_width;

                let remaining_body_rows = self.body_rows.drain(i..).collect();
                return Ok(LayoutResult::Partial(Box::new(create_remainder_table(self, remaining_body_rows))));
            }
            if let Err(e) = row.layout(&mut child_ctx, &mut occupancy, row_idx_offset + i, self.style.border_spacing) {
                log::warn!("Skipping table row that failed to lay out: {}", e);
            }
        }

        content_height_on_this_page = child_ctx.cursor.1;
        draw_background_and_borders(ctx, &self.style, block_start_y_in_ctx, content_height_on_this_page);
        ctx.cursor.1 = content_start_y_in_ctx + content_height_on_this_page + self.style.padding.bottom + border_bottom_width;
        ctx.last_v_margin = self.style.margin.bottom;

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

    fn measure(&mut self, engine: &LayoutEngine, col_widths: &[f32]) {
        self.col_widths = col_widths.to_vec();
        let mut max_height: f32 = 0.0;
        let mut col_cursor = 0;
        for cell in self.cells.iter_mut() {
            if col_cursor >= col_widths.len() { break; }
            let end_col = (col_cursor + cell.colspan).min(col_widths.len());
            let cell_width: f32 = col_widths[col_cursor..end_col].iter().sum();
            cell.measure(engine, cell_width);
            if cell.rowspan == 1 {
                max_height = max_height.max(cell.height);
            }
            col_cursor += cell.colspan;
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
                x: ctx.bounds.x + current_x,
                y: ctx.bounds.y + ctx.cursor.1,
                width: cell_width,
                height: self.height,
            };
            let mut cell_ctx = LayoutContext {
                engine: ctx.engine,
                bounds: cell_bounds,
                cursor: (0.0, 0.0),
                elements: ctx.elements,
                last_v_margin: 0.0,
                local_page_index: ctx.local_page_index,
                defined_anchors: ctx.defined_anchors,
            };
            cell.layout(&mut cell_ctx)?;

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
            content: BlockNode::new_from_children(None, children, cell_style),
            height: 0.0,
            preferred_width: 0.0,
            colspan: cell.colspan.max(1),
            rowspan: cell.rowspan.max(1),
        }
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        self.height = self.content.measure_content_height(engine, available_width);
        // Measure preferred width by giving it "infinite" space
        self.preferred_width = self.content.measure_intrinsic_width(engine);
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
    remaining_width = remaining_width.max(0.0);

    if auto_indices.is_empty() {
        return widths;
    }

    // 2. Determine preferred widths for auto columns
    let mut preferred_widths: Vec<f32> = vec![0.0; columns.len()];
    for row in header_rows.iter().chain(body_rows.iter()) {
        let mut col_cursor = 0;
        for cell in &row.cells {
            if col_cursor >= columns.len() { break; }
            if auto_indices.contains(&col_cursor) { // Simplified: only considers first col of a span
                let cell_content = cell.content.clone();
                let preferred = cell_content.measure_intrinsic_width(engine);
                if cell.colspan == 1 {
                    preferred_widths[col_cursor] = preferred_widths[col_cursor].max(preferred);
                }
            }
            col_cursor += cell.colspan;
        }
    }

    // 3. Distribute remaining width based on preferred widths
    let total_preferred: f32 = auto_indices.iter().map(|&i| preferred_widths[i]).sum();
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