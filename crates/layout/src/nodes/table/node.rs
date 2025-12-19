use crate::nodes::block::BlockNode;
use crate::style::ComputedStyle;
use crate::{
    BoxConstraints, LayoutContext, LayoutEnvironment, LayoutError, LayoutNode, LayoutResult,
    NodeState, Size,
};
use petty_idf::TableColumnDefinition;
// Use explicit geometry types from base to match Trait definition
use crate::algorithms::table_solver::{TableCellInfo, TableSolver};
use petty_types::geometry::{BoxConstraints as BaseBoxConstraints, Size as BaseSize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use super::pagination::TablePagination;

#[derive(Clone, Debug)]
pub struct TableLayoutOutput {
    pub col_widths: Vec<f32>,
    pub row_heights: Vec<f32>,
    pub total_width: f32,
    pub total_height: f32,
}

#[derive(Debug)]
pub struct TableNode<'a> {
    pub unique_id: usize,
    pub id: Option<&'a str>,
    pub header_rows: &'a [TableRowNode<'a>],
    pub body_rows: &'a [TableRowNode<'a>],
    pub style: Arc<ComputedStyle>,
    pub columns: Vec<TableColumnDefinition>,
}

impl<'a> LayoutNode for TableNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style.as_ref()
    }

    fn measure(
        &self,
        env: &LayoutEnvironment,
        constraints: BaseBoxConstraints,
    ) -> Result<BaseSize, LayoutError> {
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

        // Check generic cache in environment
        if let Some(output) = env
            .cache
            .borrow()
            .get(&key)
            .and_then(|v| v.downcast_ref::<TableLayoutOutput>())
        {
            let width = if constraints.has_bounded_width() {
                constraints.max_width
            } else {
                output.total_width
            };
            return Ok(Size::new(width, output.total_height));
        }

        let layout_output = self.compute_layout_output(env, available_width, max_height_hint)?;
        env.cache
            .borrow_mut()
            .insert(key, Box::new(layout_output.clone()));

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
        constraints: BaseBoxConstraints,
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

        let max_h = ctx.available_height() + 50.0;
        let max_height_hint = Some(max_h);

        let key = self.get_cache_key(available_width);

        let layout_output = if let Some(output) = ctx
            .env
            .cache
            .borrow()
            .get(&key)
            .and_then(|v| v.downcast_ref::<TableLayoutOutput>())
        {
            output.clone()
        } else {
            let output = self.compute_layout_output(&ctx.env, available_width, max_height_hint)?;
            ctx.env
                .cache
                .borrow_mut()
                .insert(key, Box::new(output.clone()));
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

impl<'a> TableNode<'a> {
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
        let solver = TableSolver::new(env, &self.columns);
        let all_rows = self
            .header_rows
            .iter()
            .chain(self.body_rows.iter())
            .map(|r| r.cells.iter());

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
            total_height,
        })
    }

    fn calculate_all_row_heights(
        &self,
        env: &LayoutEnvironment,
        col_widths: &[f32],
        max_height_hint: Option<f32>,
    ) -> Result<Vec<f32>, LayoutError> {
        let start = Instant::now();
        let mut row_heights = Vec::with_capacity(self.header_rows.len() + self.body_rows.len());
        let mut total_accumulated = 0.0;

        for row in self.header_rows {
            let h = row.measure_height(env, col_widths)?;
            row_heights.push(h);
            total_accumulated += h;
        }

        for row in self.body_rows {
            #[allow(clippy::collapsible_if)]
            if let Some(max_h) = max_height_hint {
                if total_accumulated > max_h {
                    row_heights.push(0.0);
                    continue;
                }
            }
            let h = row.measure_height(env, col_widths)?;
            row_heights.push(h);
            total_accumulated += h;
        }

        env.engine
            .record_perf("TableNode::calculate_all_row_heights", start.elapsed());

        Ok(row_heights)
    }
}

// Helper types
#[derive(Debug, Clone)]
pub struct TableRowNode<'a> {
    pub cells: &'a [TableCellNode<'a>],
}

impl<'a> TableRowNode<'a> {
    pub fn measure_height(
        &self,
        env: &LayoutEnvironment,
        col_widths: &[f32],
    ) -> Result<f32, LayoutError> {
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
pub struct TableCellNode<'a> {
    pub content: BlockNode<'a>,
    pub colspan: usize,
    pub rowspan: usize,
}

impl<'a> TableCellInfo for &'a TableCellNode<'a> {
    fn colspan(&self) -> usize {
        self.colspan
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

impl<'a> TableCellNode<'a> {
    pub fn measure_height(&self, env: &LayoutEnvironment, width: f32) -> Result<f32, LayoutError> {
        Ok(self
            .content
            .measure(env, BoxConstraints::tight_width(width))?
            .height)
    }
}
