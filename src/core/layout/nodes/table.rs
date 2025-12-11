use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, TableState,
};
use super::RenderNode;
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::sync::Arc;
use std::time::Instant;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Import the new solver
use super::table_solver::{TableSolver, TableCellInfo};

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

/// Helper struct to encapsulate table pagination logic.
struct TablePagination<'a, 'b> {
    node: &'a TableNode<'a>,
    ctx: &'b mut LayoutContext<'a>,
    start_row_index: usize,
    col_widths: &'a [f32],
    row_heights: &'a [f32],
    header_count: usize,
    start_y: f32,
    table_x_start: f32,
}

impl<'a, 'b> TablePagination<'a, 'b> {
    fn new(
        node: &'a TableNode<'a>,
        ctx: &'b mut LayoutContext<'a>,
        start_row_index: usize,
        col_widths: &'a [f32],
        row_heights: &'a [f32],
        start_y: f32,
        table_x_start: f32,
    ) -> Self {
        Self {
            node,
            ctx,
            start_row_index,
            col_widths,
            row_heights,
            header_count: node.header_rows.len(),
            start_y,
            table_x_start,
        }
    }

    fn run(mut self) -> Result<LayoutResult, LayoutError> {
        let mut current_y_offset = 0.0;
        let mut occupied_until_row_idx = vec![0usize; self.node.columns.len().max(1)];

        let render_start = Instant::now();

        // 1. Render Headers (if present and this is a fresh page or we repeat headers)
        // Currently we always repeat headers on break.
        if !self.node.header_rows.is_empty() {
            let mut header_occupied = vec![0usize; self.node.columns.len().max(1)];

            for (i, row) in self.node.header_rows.iter().enumerate() {
                let height = self.row_heights.get(i).copied().unwrap_or(0.0);

                self.node.render_row(
                    self.ctx,
                    row,
                    self.col_widths,
                    self.start_y + current_y_offset,
                    height,
                    self.table_x_start,
                    &mut header_occupied,
                    i,
                    &self.row_heights[i..],
                )?;
                current_y_offset += height;
            }
        }

        // 2. Render Body Rows
        for (i, row) in self.node.body_rows.iter().enumerate().skip(self.start_row_index) {
            let height_idx = self.header_count + i;
            let mut row_height = *self.row_heights.get(height_idx).unwrap_or(&0.0);

            // Fallback if height wasn't pre-calculated (unlikely with current arch)
            if row_height <= 0.001 {
                row_height = row.measure_height(&self.ctx.env, self.col_widths)?;
            }

            // Check for break
            if self.start_y + current_y_offset + row_height > self.ctx.bounds().height {
                self.ctx.env.engine.record_perf("TableNode::layout::render_rows", render_start.elapsed());
                return Ok(LayoutResult::Break(NodeState::Table(TableState {
                    row_index: i,
                })));
            }

            self.node.render_row(
                self.ctx,
                row,
                self.col_widths,
                self.start_y + current_y_offset,
                row_height,
                self.table_x_start,
                &mut occupied_until_row_idx,
                i,
                &self.row_heights[height_idx..],
            )?;
            current_y_offset += row_height;
        }

        self.ctx.env.engine.record_perf("TableNode::layout::render_rows", render_start.elapsed());

        // Finish table
        self.ctx.set_cursor_y(
            self.start_y
                + current_y_offset
                + self.node.style.box_model.padding.bottom
                + self.node.style.border_bottom_width(),
        );
        self.ctx.last_v_margin = self.node.style.box_model.margin.bottom;

        Ok(LayoutResult::Finished)
    }
}

#[derive(Debug)]
pub struct TableNode<'a> {
    /// Unique identifier for this node instance, used for stable caching.
    unique_id: usize,
    id: Option<&'a str>,
    header_rows: &'a [TableRowNode<'a>],
    body_rows: &'a [TableRowNode<'a>],
    style: Arc<ComputedStyle>,
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

        // Generate stable unique ID
        let unique_id = store.next_node_id();

        Ok(Self {
            unique_id,
            id,
            header_rows: store.bump.alloc_slice_clone(&header_vec),
            body_rows: store.bump.alloc_slice_clone(&body_vec),
            style: style_ref,
            columns: columns.clone(),
        })
    }

    fn get_cache_key(&self, available_width: Option<f32>) -> u64 {
        let mut s = DefaultHasher::new();
        self.unique_id.hash(&mut s);
        3u8.hash(&mut s); // Domain 3: Table Layout
        if let Some(w) = available_width {
            ((w * 100.0).round() as i32).hash(&mut s);
        } else {
            (-1i32).hash(&mut s);
        }
        s.finish()
    }

    fn compute_layout_output(
        &self,
        env: &LayoutEnvironment,
        available_width: Option<f32>,
        max_height_hint: Option<f32>,
    ) -> Result<TableLayoutOutput, LayoutError> {
        // Delegate width calculation to the new TableSolver
        let solver = TableSolver::new(env, &self.columns);

        // Combine header and body rows for width resolution
        let all_rows = self.header_rows.iter().chain(self.body_rows.iter()).map(|r| r.cells.iter());

        let col_widths = solver.resolve_widths(available_width, all_rows)?;
        let row_heights = self.calculate_all_row_heights(env, &col_widths, max_height_hint)?;

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

        Ok(TableLayoutOutput {
            col_widths,
            row_heights,
            total_width,
            total_height
        })
    }

    fn calculate_all_row_heights(
        &self,
        env: &LayoutEnvironment,
        col_widths: &[f32],
        max_height_hint: Option<f32>,
    ) -> Result<Vec<f32>, LayoutError> {
        let start = Instant::now();
        let mut row_measure_time = std::time::Duration::ZERO;

        let mut row_heights = Vec::with_capacity(self.header_rows.len() + self.body_rows.len());
        let mut total_accumulated = 0.0;

        for row in self.header_rows {
            let m_start = Instant::now();
            let h = row.measure_height(env, col_widths)?;
            row_measure_time += m_start.elapsed();

            row_heights.push(h);
            total_accumulated += h;
        }

        for row in self.body_rows {
            if let Some(max_h) = max_height_hint {
                if total_accumulated > max_h {
                    row_heights.push(0.0);
                    continue;
                }
            }

            let m_start = Instant::now();
            let h = row.measure_height(env, col_widths)?;
            row_measure_time += m_start.elapsed();

            row_heights.push(h);
            total_accumulated += h;
        }

        let duration = start.elapsed();
        env.engine.record_perf("TableNode::calculate_all_row_heights", duration);
        env.engine.record_perf("TableNode::calculate_all_row_heights::measure_rows", row_measure_time);

        Ok(row_heights)
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
        self.style.as_ref()
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Result<Size, LayoutError> {
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let available_width = if constraints.has_bounded_width() {
            Some((constraints.max_width - h_deduction).max(0.0))
        } else {
            None
        };

        let max_height_hint = if constraints.has_bounded_height() {
            Some(constraints.max_height)
        } else {
            Some(3000.0)
        };

        let key = self.get_cache_key(available_width);

        let cached_output = {
            let cache = env.cache.borrow();
            cache.get(&key).and_then(|v| v.downcast_ref::<TableLayoutOutput>()).cloned()
        };

        let layout_output = if let Some(output) = cached_output {
            output
        } else {
            let output = self.compute_layout_output(env, available_width, max_height_hint)?;
            env.cache.borrow_mut().insert(key, Box::new(output.clone()));
            output
        };

        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            layout_output.total_width
        };

        Ok(Size::new(width, layout_output.total_height))
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

        // Data Resolution Phase
        let h_deduction = self.style.padding_x() + self.style.border_x();
        let available_width = if constraints.has_bounded_width() {
            Some((constraints.max_width - h_deduction).max(0.0))
        } else {
            None
        };

        let max_h = ctx.available_height() + 50.0;
        let max_height_hint = Some(max_h);

        let key = self.get_cache_key(available_width);

        let cached_output = {
            let cache = ctx.env.cache.borrow();
            cache.get(&key).and_then(|v| v.downcast_ref::<TableLayoutOutput>()).cloned()
        };

        let layout_output = if let Some(output) = cached_output {
            output
        } else {
            let output = self.compute_layout_output(&ctx.env, available_width, max_height_hint)?;
            ctx.env.cache.borrow_mut().insert(key, Box::new(output.clone()));
            output
        };

        // Pre-pagination Layout Setup
        if start_row_index == 0 {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
            ctx.advance_cursor(margin_to_add);
        }
        ctx.last_v_margin = 0.0;

        let start_y = ctx.cursor_y();
        let border_left = self.style.border_left_width();
        let padding_left = self.style.box_model.padding.left;
        let table_x_start = border_left + padding_left;

        // Pagination Phase
        let paginator = TablePagination::new(
            self,
            ctx,
            start_row_index,
            &layout_output.col_widths,
            &layout_output.row_heights,
            start_y,
            table_x_start,
        );

        paginator.run()
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

    fn measure_height(&self, env: &LayoutEnvironment, col_widths: &[f32]) -> Result<f32, LayoutError> {
        let mut max_height: f32 = 0.0;
        let mut col_cursor = 0;

        for cell in self.cells {
            if col_cursor >= col_widths.len() {
                break;
            }
            let end_col = (col_cursor + cell.colspan).min(col_widths.len());
            let cell_width: f32 = col_widths[col_cursor..end_col].iter().sum();

            let h = cell.measure_height(env, cell_width)?;
            if cell.rowspan == 1 {
                max_height = max_height.max(h);
            }
            col_cursor += cell.colspan;
        }
        Ok(max_height)
    }
}

#[derive(Debug, Clone)]
struct TableCellNode<'a> {
    content: BlockNode<'a>,
    colspan: usize,
    rowspan: usize,
}

impl<'a> TableCellInfo for &'a TableCellNode<'a> {
    fn colspan(&self) -> usize {
        self.colspan
    }

    fn measure_max_content(&self, env: &LayoutEnvironment) -> Result<f32, LayoutError> {
        TableCellNode::measure_max_content(self, env)
    }
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

    fn measure_height(&self, env: &LayoutEnvironment, width: f32) -> Result<f32, LayoutError> {
        Ok(self.content
            .measure(env, BoxConstraints::tight_width(width))?
            .height)
    }

    fn measure_max_content(&self, env: &LayoutEnvironment) -> Result<f32, LayoutError> {
        let infinite_constraint = BoxConstraints {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        };
        Ok(self.content.measure(env, infinite_constraint)?.width)
    }
}