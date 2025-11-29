// src/core/layout/nodes/table.rs

use crate::core::idf::{IRNode, TableColumnDefinition, TableRow, TextStr};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode, TableState,
};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use bumpalo::Bump;
use std::sync::Arc;

pub struct TableBuilder;

impl NodeBuilder for TableBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        TableNode::build(node, engine, parent_style, arena)
    }
}

#[derive(Debug)]
pub struct TableNode<'a> {
    id: Option<TextStr>,
    header_rows: Vec<TableRowNode<'a>>,
    body_rows: Vec<TableRowNode<'a>>,
    style: Arc<ComputedStyle>,
    columns: Vec<TableColumnDefinition>,
}

impl<'a> TableNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let node = arena.alloc(Self::new(node, engine, parent_style, arena)?);
        Ok(RenderNode::Table(node))
    }

    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<Self, LayoutError> {
        let IRNode::Table {
            meta,
            columns,
            header,
            body,
            ..
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("Table", node.kind()));
        };

        let style = engine.compute_style(
            &meta.style_sets,
            meta.style_override.as_ref(),
            &parent_style,
        );

        let header_rows = if let Some(h) = header {
            h.rows
                .iter()
                .map(|r| TableRowNode::new(r, &style, engine, arena))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        let body_rows = body
            .rows
            .iter()
            .map(|r| TableRowNode::new(r, &style, engine, arena))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            id: meta.id.clone(),
            header_rows,
            body_rows,
            style,
            columns: columns.clone(),
        })
    }

    fn calculate_column_widths(
        &self,
        env: &mut LayoutEnvironment,
        available_width: Option<f32>,
    ) -> Vec<f32> {
        let mut widths = vec![0.0; self.columns.len()];
        let mut auto_indices = Vec::new();
        let table_width = available_width.unwrap_or(f32::INFINITY);
        let mut remaining_width = table_width;
        let is_finite = table_width.is_finite();

        for (i, col) in self.columns.iter().enumerate() {
            if let Some(dim) = &col.width {
                match dim {
                    Dimension::Pt(w) => {
                        widths[i] = *w;
                        remaining_width -= *w;
                    }
                    Dimension::Percent(p) => {
                        if is_finite {
                            widths[i] = (p / 100.0) * table_width;
                            remaining_width -= widths[i];
                        } else {
                            auto_indices.push(i);
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

        let mut preferred_widths: Vec<f32> = vec![0.0f32; self.columns.len()];
        let all_rows = self.header_rows.iter().chain(self.body_rows.iter());

        for row in all_rows {
            let mut col_cursor = 0;
            for cell in &row.cells {
                if col_cursor >= self.columns.len() {
                    break;
                }
                if auto_indices.contains(&col_cursor) {
                    let preferred = cell.measure_max_content(env);
                    if cell.colspan == 1 {
                        preferred_widths[col_cursor] = preferred_widths[col_cursor].max(preferred);
                    }
                }
                col_cursor += cell.colspan;
            }
        }

        let total_preferred: f32 = auto_indices.iter().map(|&i| preferred_widths[i]).sum();

        if !is_finite {
            for &i in &auto_indices {
                widths[i] = preferred_widths[i];
            }
            return widths;
        }

        if total_preferred > 0.0 && remaining_width > total_preferred {
            let extra_space = remaining_width - total_preferred;
            for &i in &auto_indices {
                widths[i] =
                    preferred_widths[i] + extra_space * (preferred_widths[i] / total_preferred);
            }
        } else if total_preferred > 0.0 {
            let shrink_factor = remaining_width / total_preferred;
            for &i in &auto_indices {
                widths[i] = preferred_widths[i] * shrink_factor;
            }
        } else {
            let width_per_auto = remaining_width / auto_indices.len() as f32;
            for i in auto_indices {
                widths[i] = width_per_auto;
            }
        }

        widths
    }

    // Helper to calculate height of all rows ahead of time
    fn calculate_all_row_heights(
        &self,
        env: &mut LayoutEnvironment,
        col_widths: &[f32]
    ) -> Vec<f32> {
        let mut row_heights = Vec::with_capacity(self.header_rows.len() + self.body_rows.len());

        // Measure header rows first
        for row in &self.header_rows {
            row_heights.push(row.measure_height(env, col_widths));
        }

        // Measure body rows
        for row in &self.body_rows {
            row_heights.push(row.measure_height(env, col_widths));
        }
        row_heights
    }

    #[allow(clippy::too_many_arguments)]
    fn render_row(
        &self,
        ctx: &mut LayoutContext,
        row: &TableRowNode,
        widths: &[f32],
        y: f32,
        height: f32,
        x_start: f32,
        occupied_until_row_idx: &mut Vec<usize>,
        current_row_idx: usize,
        future_heights: &[f32],
    ) -> Result<(), LayoutError> {
        let mut x_offset = 0.0;
        let mut col_cursor = 0;

        let mut cell_iter = row.cells.iter();

        while col_cursor < widths.len() {
            // Check if current slot is occupied by a previous rowspan
            // If the slot is blocked UNTIL row X, and we are currently at row Y,
            // we are blocked if X > Y.
            if col_cursor < occupied_until_row_idx.len() && occupied_until_row_idx[col_cursor] > current_row_idx {
                // Skip occupied slot
                x_offset += widths[col_cursor];
                col_cursor += 1;
                continue;
            }

            // Get next cell
            if let Some(cell) = cell_iter.next() {
                let colspan = cell.colspan;
                let rowspan = cell.rowspan;

                // Calculate width
                let end_col = (col_cursor + colspan).min(widths.len());
                let cell_width: f32 = widths[col_cursor..end_col].iter().sum();

                // Calculate height (handle rowspan)
                let mut cell_height = height;
                if rowspan > 1 {
                    // Sum heights of next (rowspan-1) rows
                    // future_heights[0] is current row height
                    // future_heights[1] is next row, etc.
                    let limit = rowspan.min(future_heights.len());
                    cell_height = future_heights[0..limit].iter().sum();

                    // Mark slots as occupied for future rows.
                    // If current row is 5 and rowspan is 2, it occupies 5 and 6.
                    // So it is occupied UNTIL row 7 (index 5 + 2).
                    let free_at_index = current_row_idx + rowspan;

                    for k in 0..colspan {
                        if col_cursor + k < occupied_until_row_idx.len() {
                            occupied_until_row_idx[col_cursor + k] = free_at_index;
                        }
                    }
                }

                // Layout Cell Content
                ctx.with_child_bounds(
                    crate::core::layout::geom::Rect {
                        x: ctx.bounds().x + x_start + x_offset,
                        y: ctx.bounds().y + y,
                        width: cell_width,
                        height: cell_height
                    },
                    |cell_ctx| {
                        cell.content.layout(
                            cell_ctx,
                            BoxConstraints::tight(crate::core::layout::geom::Size::new(cell_width, cell_height)),
                            None
                        )
                    }
                )?;

                x_offset += cell_width;
                col_cursor += colspan;
            } else {
                break; // No more cells in this row definition
            }
        }
        Ok(())
    }
}

impl<'a> LayoutNode for TableNode<'a> {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let available_width = if constraints.has_bounded_width() {
            Some((constraints.max_width - h_deduction).max(0.0))
        } else {
            None
        };

        let col_widths = self.calculate_column_widths(env, available_width);

        let mut total_height = 0.0;
        for row in self.header_rows.iter().chain(self.body_rows.iter()) {
            total_height += row.measure_height(env, &col_widths);
        }

        let padding_y = self.style.padding_y();
        let border_y = self.style.border_y();
        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        let total_height = margin_y + padding_y + border_y + total_height;

        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            col_widths.iter().sum::<f32>() + h_deduction
        };

        Size::new(width, total_height)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            ctx.register_anchor(id);
        }

        let start_row_index = if let Some(state) = break_state {
            state.as_table()?.row_index
        } else {
            0
        };

        // 1. Calculate Widths
        let available_width = if constraints.has_bounded_width() {
            Some((constraints.max_width - self.style.padding_x() - self.style.border_x()).max(0.0))
        } else {
            None
        };

        // Reconstruct a temporary env for measurement
        let mut env = LayoutEnvironment {
            engine: ctx.engine,
            font_system: ctx.font_system,
            local_page_index: ctx.local_page_index,
        };
        let col_widths = self.calculate_column_widths(&mut env, available_width);

        // Pre-calculate heights for all rows to handle layout
        // Note: Indices in row_heights cover [header_rows..., body_rows...]
        let all_row_heights = self.calculate_all_row_heights(&mut env, &col_widths);

        let header_count = self.header_rows.len();

        if start_row_index == 0 {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
            ctx.advance_cursor(margin_to_add);
        }
        ctx.last_v_margin = 0.0;

        let start_y = ctx.cursor_y();
        let border_left = self.style.border_left_width();
        let padding_left = self.style.box_model.padding.left;
        let table_x_start = border_left + padding_left; // Relative to ctx bounds

        let mut current_y_offset = 0.0; // Relative to content box

        // FIX: Track occupied slots using absolute row indices.
        // `occupied_until_row_idx[col]` means column `col` is occupied until `row_idx` (exclusive).
        // e.g. if row 0 has rowspan 2, it occupies 0 and 1. It is free at 2.
        let mut occupied_until_row_idx = vec![0usize; self.columns.len().max(1)];

        // 2. Draw Header
        // Headers are typically an independent grid from the body.
        // We render headers first.
        if !self.header_rows.is_empty() {
            for (i, row) in self.header_rows.iter().enumerate() {
                let height = all_row_heights[i];

                // Headers have their own rowspan context
                let mut header_occupied = vec![0usize; self.columns.len().max(1)];

                self.render_row(
                    ctx,
                    row,
                    &col_widths,
                    start_y + current_y_offset,
                    height,
                    table_x_start,
                    &mut header_occupied,
                    i, // Current row index within header context
                    &all_row_heights // Pass all heights
                )?;
                current_y_offset += height;
            }
        }

        // 3. Body Rows
        // Note: If we resumed from a page break, `occupied_until_row_idx` would be lost.
        // Recovering exact rowspan state across page breaks is complex. 
        // For now, we assume clean breaks or simple layouts.

        for (i, row) in self.body_rows.iter().enumerate().skip(start_row_index) {
            // Index in all_row_heights includes headers
            let height_idx = header_count + i;
            let row_height = all_row_heights[height_idx];

            if start_y + current_y_offset + row_height > ctx.bounds().height {
                // Break here
                return Ok(LayoutResult::Break(NodeState::Table(TableState { row_index: i })));
            }

            self.render_row(
                ctx,
                row,
                &col_widths,
                start_y + current_y_offset,
                row_height,
                table_x_start,
                &mut occupied_until_row_idx,
                i, // Current body row index
                &all_row_heights[height_idx..] // pass slice starting from current row for height lookahead
            )?;
            current_y_offset += row_height;
        }

        ctx.set_cursor_y(start_y + current_y_offset + self.style.box_model.padding.bottom + self.style.border_bottom_width());
        ctx.last_v_margin = self.style.box_model.margin.bottom;

        Ok(LayoutResult::Finished)
    }
}

#[derive(Debug)]
struct TableRowNode<'a> {
    cells: Vec<TableCellNode<'a>>,
}

impl<'a> TableRowNode<'a> {
    fn new(
        row: &TableRow,
        style: &Arc<ComputedStyle>,
        engine: &LayoutEngine,
        arena: &'a Bump,
    ) -> Result<Self, LayoutError> {
        let cells = row
            .cells
            .iter()
            .map(|c| TableCellNode::new(c, style, engine, arena))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { cells })
    }

    fn measure_height(&self, env: &mut LayoutEnvironment, col_widths: &[f32]) -> f32 {
        let mut max_height: f32 = 0.0;
        let mut col_cursor = 0;

        for cell in &self.cells {
            if col_cursor >= col_widths.len() {
                break;
            }
            let end_col = (col_cursor + cell.colspan).min(col_widths.len());
            let cell_width: f32 = col_widths[col_cursor..end_col].iter().sum();

            let h = cell.measure_height(env, cell_width);
            if cell.rowspan == 1 {
                max_height = max_height.max(h);
            }
            col_cursor += cell.colspan;
        }
        max_height
    }
}

#[derive(Debug)]
struct TableCellNode<'a> {
    content: BlockNode<'a>,
    colspan: usize,
    rowspan: usize,
}

impl<'a> TableCellNode<'a> {
    fn new(
        cell: &crate::core::idf::TableCell,
        style: &Arc<ComputedStyle>,
        engine: &LayoutEngine,
        arena: &'a Bump,
    ) -> Result<Self, LayoutError> {
        let cell_style = engine.compute_style(&cell.style_sets, cell.style_override.as_ref(), style);

        let mut children = Vec::new();
        for c in &cell.children {
            children.push(engine.build_layout_node_tree(c, cell_style.clone(), arena)?);
        }

        Ok(Self {
            content: BlockNode::new_from_children(None, children, cell_style, arena),
            colspan: cell.col_span.max(1),
            rowspan: cell.row_span.max(1),
        })
    }

    fn measure_height(&self, env: &mut LayoutEnvironment, width: f32) -> f32 {
        self.content.measure(env, BoxConstraints::tight_width(width)).height
    }

    fn measure_max_content(&self, env: &mut LayoutEnvironment) -> f32 {
        let infinite_constraint = BoxConstraints {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        };
        self.content.measure(env, infinite_constraint).width
    }
}