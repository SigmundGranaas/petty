// src/core/layout/nodes/table.rs

use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode, TableState,
};
use crate::core::layout::nodes::block::{create_background_and_borders, BlockNode};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

#[derive(Debug)]
pub struct TableNode {
    id: Option<String>,
    header_rows: Vec<TableRowNode>,
    body_rows: Vec<TableRowNode>,
    style: Arc<ComputedStyle>,
    columns: Vec<TableColumnDefinition>,
}

impl TableNode {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::Table(Box::new(Self::new(
            node,
            engine,
            parent_style,
        )?)))
    }

    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
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
                .map(|r| TableRowNode::new(r, &style, engine))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        let body_rows = body
            .rows
            .iter()
            .map(|r| TableRowNode::new(r, &style, engine))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            id: meta.id.clone(),
            header_rows,
            body_rows,
            style,
            columns: columns.clone(),
        })
    }

    /// Calculates column widths based on constraints and content.
    fn calculate_column_widths(
        &self,
        env: &LayoutEnvironment,
        available_width: Option<f32>,
    ) -> Vec<f32> {
        let mut widths = vec![0.0; self.columns.len()];
        let mut auto_indices = Vec::new();
        let table_width = available_width.unwrap_or(f32::INFINITY);
        let mut remaining_width = table_width;
        let is_finite = table_width.is_finite();

        // 1. Resolve definite widths
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

        // 2. Measure max content for auto columns
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

        // 3. Distribute remaining space
        let total_preferred: f32 = auto_indices.iter().map(|&i| preferred_widths[i]).sum();

        if !is_finite {
            for &i in &auto_indices {
                widths[i] = preferred_widths[i];
            }
            return widths;
        }

        if total_preferred > 0.0 && remaining_width > total_preferred {
            // Expand
            let extra_space = remaining_width - total_preferred;
            for &i in &auto_indices {
                widths[i] =
                    preferred_widths[i] + extra_space * (preferred_widths[i] / total_preferred);
            }
        } else if total_preferred > 0.0 {
            // Shrink
            let shrink_factor = remaining_width / total_preferred;
            for &i in &auto_indices {
                widths[i] = preferred_widths[i] * shrink_factor;
            }
        } else {
            // Equal distribution
            let width_per_auto = remaining_width / auto_indices.len() as f32;
            for i in auto_indices {
                widths[i] = width_per_auto;
            }
        }

        widths
    }
}

impl LayoutNode for TableNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
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
        let is_continuation = start_row_index > 0;

        let h_deduction = self.style.padding_x() + self.style.border_x();
        let available_width = if constraints.has_bounded_width() {
            Some((constraints.max_width - h_deduction).max(0.0))
        } else {
            None
        };

        let env = LayoutEnvironment {
            engine: ctx.engine,
            local_page_index: ctx.local_page_index,
        };

        let col_widths = self.calculate_column_widths(&env, available_width);

        let margin_to_add = if !is_continuation {
            self.style.box_model.margin.top.max(ctx.last_v_margin)
        } else {
            0.0
        };
        ctx.advance_cursor(margin_to_add);
        ctx.last_v_margin = 0.0;

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();
        let border_left = self.style.border_left_width();

        let top_spacing = if !is_continuation {
            border_top + self.style.box_model.padding.top
        } else {
            0.0
        };

        let block_start_y = ctx.cursor_y();
        ctx.advance_cursor(top_spacing);
        let content_start_y = ctx.cursor_y();

        let ctx_bounds = ctx.bounds();
        let child_bounds = geom::Rect {
            x: ctx_bounds.x + border_left + self.style.box_model.padding.left,
            y: ctx_bounds.y + content_start_y,
            width: ctx_bounds.width - self.style.padding_x() - self.style.border_x(),
            height: ctx.available_height(),
        };

        let num_cols = self.columns.len();
        let num_rows = self.header_rows.len() + self.body_rows.len();
        let mut occupancy = vec![vec![false; num_cols]; num_rows];

        let mut break_occurred = false;
        let mut next_row_index = 0;

        let used_height = ctx.with_child_bounds(child_bounds, |child_ctx| {
            // Layout Header
            for (i, row) in self.header_rows.iter().enumerate() {
                let res = row.layout(
                    child_ctx,
                    &mut occupancy,
                    i,
                    self.style.table.border_spacing,
                    &col_widths,
                )?;
                if let LayoutResult::Break(_) = res {
                    break_occurred = true;
                    // If header breaks, we actually restart body rows from index, header repeats on next page usually
                    next_row_index = start_row_index;
                    return Ok(child_ctx.cursor_y());
                }
            }

            let row_offset = self.header_rows.len();

            // Layout Body
            for (i, row) in self.body_rows.iter().enumerate().skip(start_row_index) {
                let res = row.layout(
                    child_ctx,
                    &mut occupancy,
                    row_offset + i,
                    self.style.table.border_spacing,
                    &col_widths,
                )?;
                if let LayoutResult::Break(_) = res {
                    break_occurred = true;
                    next_row_index = i;
                    return Ok(child_ctx.cursor_y());
                }
            }
            Ok(child_ctx.cursor_y())
        })?;

        let bg_elements = create_background_and_borders(
            ctx.bounds(),
            &self.style,
            block_start_y,
            used_height,
            !is_continuation,
            !break_occurred,
        );
        for el in bg_elements {
            ctx.push_element(el);
        }

        if break_occurred {
            ctx.set_cursor_y(content_start_y + used_height);
            Ok(LayoutResult::Break(NodeState::Table(TableState {
                row_index: next_row_index,
            })))
        } else {
            let bottom_spacing = self.style.box_model.padding.bottom + border_bottom;
            ctx.set_cursor_y(content_start_y + used_height + bottom_spacing);
            ctx.last_v_margin = self.style.box_model.margin.bottom;
            Ok(LayoutResult::Finished)
        }
    }
}

#[derive(Debug)]
pub struct TableRowNode {
    cells: Vec<TableCellNode>,
}

impl TableRowNode {
    fn new(
        row: &TableRow,
        style: &Arc<ComputedStyle>,
        engine: &LayoutEngine,
    ) -> Result<Self, LayoutError> {
        let cells = row
            .cells
            .iter()
            .map(|c| TableCellNode::new(c, style, engine))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { cells })
    }

    fn measure_height(&self, env: &LayoutEnvironment, col_widths: &[f32]) -> f32 {
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

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        occupancy: &mut [Vec<bool>],
        row_idx: usize,
        border_spacing: f32,
        col_widths: &[f32],
    ) -> Result<LayoutResult, LayoutError> {
        let env = LayoutEnvironment {
            engine: ctx.engine,
            local_page_index: ctx.local_page_index,
        };
        let row_height = self.measure_height(&env, col_widths);

        if row_height > ctx.available_height() && !ctx.is_empty() {
            return Ok(LayoutResult::Break(NodeState::Atomic));
        }

        let mut current_x = 0.0;
        let mut col_idx = 0;

        for cell in &self.cells {
            // Skip occupied columns (handled by rowspan from previous rows)
            while col_idx < col_widths.len()
                && occupancy
                .get(row_idx)
                .and_then(|r| r.get(col_idx))
                .map_or(false, |&o| o)
            {
                current_x += col_widths[col_idx] + border_spacing;
                col_idx += 1;
            }

            if col_idx >= col_widths.len() {
                break;
            }

            let cell_col_idx = col_idx;
            let cell_width = (0..cell.colspan)
                .map(|i| col_widths.get(cell_col_idx + i).copied().unwrap_or(0.0))
                .sum::<f32>()
                + (cell.colspan.saturating_sub(1) as f32 * border_spacing);

            let cell_bounds = geom::Rect {
                x: ctx.bounds().x + current_x,
                y: ctx.bounds().y + ctx.cursor_y(),
                width: cell_width,
                height: row_height,
            };

            ctx.with_child_bounds(cell_bounds, |cell_ctx| cell.layout(cell_ctx))?;

            // Mark occupancy for rowspans
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

        ctx.advance_cursor(row_height + border_spacing);
        Ok(LayoutResult::Finished)
    }
}

#[derive(Debug)]
struct TableCellNode {
    content: BlockNode,
    colspan: usize,
    rowspan: usize,
}

impl TableCellNode {
    fn new(
        cell: &crate::core::idf::TableCell,
        style: &Arc<ComputedStyle>,
        engine: &LayoutEngine,
    ) -> Result<Self, LayoutError> {
        let cell_style =
            engine.compute_style(&cell.style_sets, cell.style_override.as_ref(), style);

        let mut children = Vec::new();
        for c in &cell.children {
            children.push(engine.build_layout_node_tree(c, cell_style.clone())?);
        }

        Ok(Self {
            content: BlockNode::new_from_children(None, children, cell_style),
            colspan: cell.col_span.max(1),
            rowspan: cell.row_span.max(1),
        })
    }

    fn measure_height(&self, env: &LayoutEnvironment, width: f32) -> f32 {
        self.content
            .measure(env, BoxConstraints::tight_width(width))
            .height
    }

    fn measure_max_content(&self, env: &LayoutEnvironment) -> f32 {
        let infinite_constraint = BoxConstraints {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        };
        self.content.measure(env, infinite_constraint).width
    }

    fn layout(&self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        self.content
            .layout(ctx, BoxConstraints::tight_width(ctx.bounds().width), None)
    }
}