// src/core/layout/nodes/table.rs

use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode, TableState,
};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use crate::core::style::dimension::Dimension;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

pub struct TableBuilder;

impl NodeBuilder for TableBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        TableNode::build(node, engine, parent_style, store)
    }
}

#[derive(Clone, Debug)]
struct TableLayoutOutput {
    col_widths: Vec<f32>,
    row_heights: Vec<f32>,
    total_width: f32,
    total_height: f32,
}

#[derive(Debug)]
pub struct TableNode<'a> {
    id: Option<&'a str>,
    header_rows: &'a [TableRowNode<'a>],
    body_rows: &'a [TableRowNode<'a>],
    style: &'a ComputedStyle,
    columns: Vec<TableColumnDefinition>,
}

impl<'a> TableNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let node = store.bump.alloc(Self::new(node, engine, parent_style, store)?);
        Ok(RenderNode::Table(node))
    }

    pub fn new(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
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

        let header_vec = if let Some(h) = header {
            h.rows
                .iter()
                .map(|r| TableRowNode::new(r, &style, engine, store))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        let body_vec = body
            .rows
            .iter()
            .map(|r| TableRowNode::new(r, &style, engine, store))
            .collect::<Result<Vec<_>, _>>()?;

        let id = meta.id.as_ref().map(|s| store.alloc_str(s));
        let style_ref = store.cache_style(style);

        Ok(Self {
            id,
            header_rows: store.bump.alloc_slice_clone(&header_vec),
            body_rows: store.bump.alloc_slice_clone(&body_vec),
            style: style_ref,
            columns: columns.clone(),
        })
    }

    fn get_cache_key(&self, available_width: Option<f32>) -> u64 {
        // Optimized: Avoid DefaultHasher overhead for pointer hashing
        let ptr_val = (self as *const Self) as u64;
        let salt = 200;

        let width_part = if let Some(w) = available_width {
            (w * 100.0).round() as u64
        } else {
            0
        };

        // Note: We deliberately exclude max_height_hint from the cache key.
        // The column widths depend only on width.
        // The row heights are intrinsic.
        // If a previous pass calculated row heights up to a large height (e.g. measure pass),
        // we want to reuse that work for the layout pass even if the layout pass has a tighter height constraint.
        ptr_val.wrapping_add(salt).wrapping_mul(33) ^ width_part
    }

    fn compute_layout_output(
        &self,
        env: &mut LayoutEnvironment,
        available_width: Option<f32>,
        max_height_hint: Option<f32>,
    ) -> TableLayoutOutput {
        let col_widths = self.calculate_column_widths(env, available_width);

        // Pass the max_height hint to stop calculating rows if we blow the page budget
        let row_heights = self.calculate_all_row_heights(env, &col_widths, max_height_hint);

        let padding_y = self.style.padding_y();
        let border_y = self.style.border_y();
        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        let content_height: f32 = row_heights.iter().sum();
        let total_height = margin_y + padding_y + border_y + content_height;

        let h_deduction = self.style.padding_x() + self.style.border_x();

        let total_width = if let Some(_w) = available_width {
            col_widths.iter().sum::<f32>() + h_deduction
        } else {
            col_widths.iter().sum::<f32>() + h_deduction
        };

        TableLayoutOutput {
            col_widths,
            row_heights,
            total_width,
            total_height
        }
    }

    fn calculate_column_widths(
        &self,
        env: &mut LayoutEnvironment,
        available_width: Option<f32>,
    ) -> Vec<f32> {
        let start = Instant::now();
        let mut measure_time = std::time::Duration::ZERO;

        let mut widths = vec![0.0; self.columns.len()];
        let mut auto_indices = Vec::new();
        let table_width = available_width.unwrap_or(0.0);
        let mut remaining_width = table_width;

        let is_finite = available_width.is_some();

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
            let duration = start.elapsed();
            env.engine.record_perf("TableNode::calculate_column_widths", duration);
            return widths;
        }

        let mut preferred_widths: Vec<f32> = vec![0.0f32; self.columns.len()];
        let all_rows = self.header_rows.iter().chain(self.body_rows.iter());

        // OPTIMIZATION: Max Rows Sampling.
        // Don't scan 10,000 rows to determine column widths. Sample the first 100.
        // This makes table layout O(1) instead of O(N) relative to row count.
        const AUTO_LAYOUT_SAMPLE_LIMIT: usize = 100;

        for (row_idx, row) in all_rows.enumerate() {
            if row_idx >= AUTO_LAYOUT_SAMPLE_LIMIT {
                break;
            }

            let mut col_cursor = 0;
            for cell in row.cells {
                if col_cursor >= self.columns.len() {
                    break;
                }

                let involves_auto_col = (col_cursor..(col_cursor + cell.colspan))
                    .any(|idx| auto_indices.contains(&idx));

                if involves_auto_col {
                    let m_start = Instant::now();
                    let preferred = cell.measure_max_content(env);
                    measure_time += m_start.elapsed();

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

        let duration = start.elapsed();
        env.engine.record_perf("TableNode::calculate_column_widths", duration);
        env.engine.record_perf("TableNode::calculate_column_widths::measure_content", measure_time);

        widths
    }

    fn calculate_all_row_heights(
        &self,
        env: &mut LayoutEnvironment,
        col_widths: &[f32],
        max_height_hint: Option<f32>,
    ) -> Vec<f32> {
        let start = Instant::now();
        let mut row_measure_time = std::time::Duration::ZERO;

        let mut row_heights = Vec::with_capacity(self.header_rows.len() + self.body_rows.len());
        let mut total_accumulated = 0.0;

        for row in self.header_rows {
            let m_start = Instant::now();
            let h = row.measure_height(env, col_widths);
            row_measure_time += m_start.elapsed();

            row_heights.push(h);
            total_accumulated += h;
        }

        for row in self.body_rows {
            if let Some(max_h) = max_height_hint {
                if total_accumulated > max_h {
                    // Optimization: stop measuring if we exceed the page height (or the cap).
                    // Push 0.0 as placeholder.
                    row_heights.push(0.0);
                    continue;
                }
            }

            let m_start = Instant::now();
            let h = row.measure_height(env, col_widths);
            row_measure_time += m_start.elapsed();

            row_heights.push(h);
            total_accumulated += h;
        }

        let duration = start.elapsed();
        env.engine.record_perf("TableNode::calculate_all_row_heights", duration);
        env.engine.record_perf("TableNode::calculate_all_row_heights::measure_rows", row_measure_time);

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
            if col_cursor < occupied_until_row_idx.len()
                && occupied_until_row_idx[col_cursor] > current_row_idx
            {
                x_offset += widths[col_cursor];
                col_cursor += 1;
                continue;
            }

            if let Some(cell) = cell_iter.next() {
                let colspan = cell.colspan;
                let rowspan = cell.rowspan;

                let end_col = (col_cursor + colspan).min(widths.len());
                let cell_width: f32 = widths[col_cursor..end_col].iter().sum();

                let mut cell_height = height;
                if rowspan > 1 {
                    let limit = rowspan.min(future_heights.len());
                    cell_height = future_heights[0..limit].iter().sum();

                    let free_at_index = current_row_idx + rowspan;

                    for k in 0..colspan {
                        if col_cursor + k < occupied_until_row_idx.len() {
                            occupied_until_row_idx[col_cursor + k] = free_at_index;
                        }
                    }
                }

                let cell_rect = crate::core::layout::geom::Rect {
                    x: ctx.bounds().x + x_start + x_offset,
                    y: ctx.bounds().y + y,
                    width: cell_width,
                    height: cell_height,
                };

                let mut cell_ctx = ctx.child(cell_rect);
                // Note: cell.content is a BlockNode
                cell.content.layout(
                    &mut cell_ctx,
                    BoxConstraints::tight(crate::core::layout::geom::Size::new(
                        cell_width,
                        cell_height,
                    )),
                    None,
                )?;

                x_offset += cell_width;
                col_cursor += colspan;
            } else {
                break;
            }
        }
        Ok(())
    }
}

impl<'a> LayoutNode for TableNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style
    }

    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let available_width = if constraints.has_bounded_width() {
            Some((constraints.max_width - h_deduction).max(0.0))
        } else {
            None
        };

        // OPTIMIZATION: Cap height measurement for unbound constraints
        // to avoid full table scan (O(N)) on large tables.
        // We use 3000.0 pt (~4 pages) as a "large enough" threshold.
        // If the table is larger, the specific height doesn't matter for flex container main size
        // logic in standard document flows, as pagination will handle the breaks.
        let max_height_hint = if constraints.has_bounded_height() {
            Some(constraints.max_height)
        } else {
            Some(3000.0)
        };

        // CACHING LOGIC
        // We do NOT include max_height_hint in the key, so that measure and layout can share the cache.
        let key = self.get_cache_key(available_width);

        let layout_output = if let Some(cached) = env.cache.get(&key) {
            if let Some(output) = cached.downcast_ref::<TableLayoutOutput>() {
                output.clone()
            } else {
                let output = self.compute_layout_output(env, available_width, max_height_hint);
                env.cache.insert(key, Box::new(output.clone()));
                output
            }
        } else {
            let output = self.compute_layout_output(env, available_width, max_height_hint);
            env.cache.insert(key, Box::new(output.clone()));
            output
        };

        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            layout_output.total_width
        };

        Size::new(width, layout_output.total_height)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = self.id {
            ctx.register_anchor(id);
        }

        let start_row_index = if let Some(state) = break_state {
            state.as_table()?.row_index
        } else {
            0
        };

        let h_deduction = self.style.padding_x() + self.style.border_x();
        let available_width = if constraints.has_bounded_width() {
            Some((constraints.max_width - h_deduction).max(0.0))
        } else {
            None
        };

        let max_h = ctx.available_height() + 50.0; // Buffer for margin errors
        let max_height_hint = Some(max_h);

        // CACHING LOGIC
        // We use the same key derivation (excluding height hint).
        // If 'measure' ran before with a huge hint (3000), we reuse that work.
        let key = self.get_cache_key(available_width);

        let layout_output = if let Some(cached) = ctx.env.cache.get(&key) {
            if let Some(output) = cached.downcast_ref::<TableLayoutOutput>() {
                output.clone()
            } else {
                let output = self.compute_layout_output(&mut ctx.env, available_width, max_height_hint);
                ctx.env.cache.insert(key, Box::new(output.clone()));
                output
            }
        } else {
            let output = self.compute_layout_output(&mut ctx.env, available_width, max_height_hint);
            ctx.env.cache.insert(key, Box::new(output.clone()));
            output
        };

        let col_widths = layout_output.col_widths;
        let all_row_heights = layout_output.row_heights;

        let header_count = self.header_rows.len();

        if start_row_index == 0 {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
            ctx.advance_cursor(margin_to_add);
        }
        ctx.last_v_margin = 0.0;

        let start_y = ctx.cursor_y();
        let border_left = self.style.border_left_width();
        let padding_left = self.style.box_model.padding.left;
        let table_x_start = border_left + padding_left;

        let mut current_y_offset = 0.0;
        let mut occupied_until_row_idx = vec![0usize; self.columns.len().max(1)];

        let render_start = Instant::now();
        if !self.header_rows.is_empty() {
            let mut header_occupied = vec![0usize; self.columns.len().max(1)];

            for (i, row) in self.header_rows.iter().enumerate() {
                let height = all_row_heights.get(i).copied().unwrap_or(0.0);

                self.render_row(
                    ctx,
                    row,
                    &col_widths,
                    start_y + current_y_offset,
                    height,
                    table_x_start,
                    &mut header_occupied,
                    i,
                    &all_row_heights[i..],
                )?;
                current_y_offset += height;
            }
        }

        for (i, row) in self.body_rows.iter().enumerate().skip(start_row_index) {
            let height_idx = header_count + i;

            // If we deferred measurement due to page-break optimization (or cache mismatch in hint size),
            // the height might be 0.0 or missing.
            let mut row_height = *all_row_heights.get(height_idx).unwrap_or(&0.0);

            if row_height <= 0.001 {
                // Late binding measure because we skipped it earlier (or previous pass stopped early)
                row_height = row.measure_height(&mut ctx.env, &col_widths);
            }

            if start_y + current_y_offset + row_height > ctx.bounds().height {
                return Ok(LayoutResult::Break(NodeState::Table(TableState {
                    row_index: i,
                })));
            }

            self.render_row(
                ctx,
                row,
                &col_widths,
                start_y + current_y_offset,
                row_height,
                table_x_start,
                &mut occupied_until_row_idx,
                i,
                &all_row_heights[height_idx..],
            )?;
            current_y_offset += row_height;
        }
        ctx.env.engine.record_perf("TableNode::layout::render_rows", render_start.elapsed());

        ctx.set_cursor_y(
            start_y
                + current_y_offset
                + self.style.box_model.padding.bottom
                + self.style.border_bottom_width(),
        );
        ctx.last_v_margin = self.style.box_model.margin.bottom;

        Ok(LayoutResult::Finished)
    }
}

#[derive(Debug, Clone)]
struct TableRowNode<'a> {
    cells: &'a [TableCellNode<'a>],
}

impl<'a> TableRowNode<'a> {
    fn new(
        row: &TableRow,
        style: &Arc<ComputedStyle>,
        engine: &LayoutEngine,
        store: &'a LayoutStore,
    ) -> Result<Self, LayoutError> {
        let cells = row
            .cells
            .iter()
            .map(|c| TableCellNode::new(c, style, engine, store))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { cells: store.bump.alloc_slice_clone(&cells) })
    }

    fn measure_height(&self, env: &mut LayoutEnvironment, col_widths: &[f32]) -> f32 {
        let mut max_height: f32 = 0.0;
        let mut col_cursor = 0;

        for cell in self.cells {
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

#[derive(Debug, Clone)]
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
        store: &'a LayoutStore,
    ) -> Result<Self, LayoutError> {
        let cell_style =
            engine.compute_style(&cell.style_sets, cell.style_override.as_ref(), style);

        let mut children = Vec::new();
        for c in &cell.children {
            children.push(engine.build_layout_node_tree(c, cell_style.clone(), store)?);
        }

        Ok(Self {
            content: BlockNode::new_from_children(None, children, cell_style, store),
            colspan: cell.col_span.max(1),
            rowspan: cell.row_span.max(1),
        })
    }

    fn measure_height(&self, env: &mut LayoutEnvironment, width: f32) -> f32 {
        self.content
            .measure(env, BoxConstraints::tight_width(width))
            .height
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