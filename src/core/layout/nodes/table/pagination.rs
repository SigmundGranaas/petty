use crate::core::layout::{LayoutContext, LayoutError, LayoutNode, LayoutResult, NodeState, TableState};
use super::node::TableNode;
use crate::core::layout::{BoxConstraints, Size, Rect};

/// encapsulated table pagination logic.
pub struct TablePagination<'node, 'ctx, 'data, 'a, 'b> {
    node: &'node TableNode<'a>,
    ctx: &'ctx mut LayoutContext<'b>,
    start_row_index: usize,
    col_widths: &'data [f32],
    row_heights: &'data [f32],
    start_y: f32,
    table_x_start: f32,
    header_count: usize,
}

impl<'node, 'ctx, 'data, 'a, 'b> TablePagination<'node, 'ctx, 'data, 'a, 'b> {
    pub fn new(
        node: &'node TableNode<'a>,
        ctx: &'ctx mut LayoutContext<'b>,
        start_row_index: usize,
        col_widths: &'data [f32],
        row_heights: &'data [f32],
        start_y: f32,
        table_x_start: f32,
    ) -> Self {
        Self {
            node,
            ctx,
            start_row_index,
            col_widths,
            row_heights,
            start_y,
            table_x_start,
            header_count: node.header_rows.len(),
        }
    }

    pub fn run(mut self) -> Result<LayoutResult, LayoutError> {
        let mut current_y_offset = 0.0;
        let mut occupied_until_row_idx = vec![0usize; self.node.columns.len().max(1)];

        // 1. Render Headers
        if !self.node.header_rows.is_empty() {
            let mut header_occupied = vec![0usize; self.node.columns.len().max(1)];

            for (i, row) in self.node.header_rows.iter().enumerate() {
                let height = self.row_heights.get(i).copied().unwrap_or(0.0);
                self.render_row(
                    row,
                    self.start_y + current_y_offset,
                    height,
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
            let row_height = *self.row_heights.get(height_idx).unwrap_or(&0.0);

            if self.start_y + current_y_offset + row_height > self.ctx.bounds().height {
                return Ok(LayoutResult::Break(NodeState::Table(TableState {
                    row_index: i,
                })));
            }

            self.render_row(
                row,
                self.start_y + current_y_offset,
                row_height,
                &mut occupied_until_row_idx,
                i,
                &self.row_heights[height_idx..],
            )?;
            current_y_offset += row_height;
        }

        self.ctx.set_cursor_y(
            self.start_y
                + current_y_offset
                + self.node.style.box_model.padding.bottom
                + self.node.style.border_bottom_width(),
        );
        self.ctx.last_v_margin = self.node.style.box_model.margin.bottom;

        Ok(LayoutResult::Finished)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_row(
        &mut self,
        row: &super::node::TableRowNode,
        y: f32,
        height: f32,
        occupied_until_row_idx: &mut Vec<usize>,
        current_row_idx: usize,
        future_heights: &[f32],
    ) -> Result<(), LayoutError> {
        let mut x_offset = 0.0;
        let mut col_cursor = 0;
        let mut cell_iter = row.cells.iter();

        while col_cursor < self.col_widths.len() {
            if col_cursor < occupied_until_row_idx.len() && occupied_until_row_idx[col_cursor] > current_row_idx {
                x_offset += self.col_widths[col_cursor];
                col_cursor += 1;
                continue;
            }

            if let Some(cell) = cell_iter.next() {
                let colspan = cell.colspan;
                let rowspan = cell.rowspan;

                let end_col = (col_cursor + colspan).min(self.col_widths.len());
                let cell_width: f32 = self.col_widths[col_cursor..end_col].iter().sum();

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

                let cell_rect = Rect {
                    x: self.ctx.bounds().x + self.table_x_start + x_offset,
                    y: self.ctx.bounds().y + y,
                    width: cell_width,
                    height: cell_height,
                };

                let mut cell_ctx = self.ctx.child(cell_rect);
                cell.content.layout(
                    &mut cell_ctx,
                    BoxConstraints::tight(Size::new(cell_width, cell_height)),
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