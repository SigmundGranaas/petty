use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{self, BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::nodes::block::{draw_background_and_borders, BlockNode};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use std::sync::Arc;

pub struct TableBuilder;

impl NodeBuilder for TableBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::Table(TableNode::new(node, engine, parent_style)?))
    }
}

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
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        let (meta, columns, header, body) = match node {
            IRNode::Table {
                meta,
                columns,
                header,
                body,
                ..
            } => (meta, columns, header, body),
            _ => return Err(LayoutError::BuilderMismatch("Table", node.kind())),
        };

        let style =
            engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), &parent_style);

        let header_rows = if let Some(h) = header {
            h.rows.iter().map(|r| TableRowNode::new(r, &style, engine)).collect::<Result<Vec<_>,_>>()?
        } else {
            Vec::new()
        };

        let body_rows = body.rows.iter().map(|r| TableRowNode::new(r, &style, engine)).collect::<Result<Vec<_>,_>>()?;

        Ok(Self {
            id: meta.id.clone(),
            header_rows,
            body_rows,
            style,
            columns: columns.clone(),
            calculated_widths: Vec::new(),
        })
    }
}

impl LayoutNode for TableNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let h_deduction = self.style.padding_x() + self.style.border_x();

        let available_width = if constraints.has_bounded_width() {
            (constraints.max_width - h_deduction).max(0.0)
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

        let padding_y = self.style.padding_y();
        let border_y = self.style.border_y();

        let total_height = self.style.box_model.margin.top
            + padding_y
            + border_y
            + total_height
            + self.style.box_model.margin.bottom;

        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            self.calculated_widths.iter().sum::<f32>() + h_deduction
        };

        Size::new(width, total_height)
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutContext,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }

        let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
        if !ctx.is_empty() && margin_to_add > ctx.available_height() {
            return Ok(LayoutResult::Partial(RenderNode::Table(self.clone())));
        }
        ctx.advance_cursor(margin_to_add);
        ctx.last_v_margin = 0.0;

        let border_top = self.style.border_top_width();
        let border_bottom = self.style.border_bottom_width();
        let border_left = self.style.border_left_width();
        let _border_right = self.style.border_right_width();

        let block_start_y_in_ctx = ctx.cursor.1;
        ctx.advance_cursor(border_top + self.style.box_model.padding.top);
        let content_start_y_in_ctx = ctx.cursor.1;

        let child_bounds = geom::Rect {
            x: ctx.bounds.x + border_left + self.style.box_model.padding.left,
            y: ctx.bounds.y + content_start_y_in_ctx,
            width: ctx.bounds.width
                - self.style.padding_x()
                - self.style.border_x(),
            height: ctx.available_height(),
        };

        // We use with_child_bounds to layout table rows, but table logic is complex (occupancy matrix).
        // It's cleaner to handle occupancy logic here and pass a restricted context to each row.

        let num_cols = self.columns.len();
        let num_rows = self.header_rows.len() + self.body_rows.len();
        let mut occupancy = vec![vec![false; num_cols]; num_rows];
        let mut row_idx_offset = 0;
        let mut content_height_on_this_page = 0.0;

        // Use child context for rows to simplify relative cursor math
        let mut split_occurred = false;
        let mut remaining_body_rows = Vec::new();

        ctx.with_child_bounds(child_bounds, |child_ctx| {
            for (i, row) in self.header_rows.iter_mut().enumerate() {
                if row.height > child_ctx.bounds.height {
                    // Fail if a single row is too tall for page
                    return Err(LayoutError::ElementTooLarge(
                        row.height,
                        child_ctx.bounds.height,
                    ));
                }
                if row.height > child_ctx.available_height() && !child_ctx.is_empty() {
                    split_occurred = true;
                    // If header doesn't fit, we break immediately
                    remaining_body_rows = self.body_rows.drain(..).collect();
                    return Ok(());
                }
                if let Err(e) = row.layout(
                    child_ctx,
                    &mut occupancy,
                    i,
                    self.style.table.border_spacing,
                ) {
                    log::warn!("Skipping table header row that failed to lay out: {}", e);
                }
            }
            content_height_on_this_page += child_ctx.cursor.1;
            row_idx_offset += self.header_rows.len();

            for (i, row) in self.body_rows.iter_mut().enumerate() {
                if split_occurred { break; }

                if row.height > child_ctx.bounds.height {
                    return Err(LayoutError::ElementTooLarge(
                        row.height,
                        child_ctx.bounds.height,
                    ));
                }
                if row.height > child_ctx.available_height() && !child_ctx.is_empty() {
                    split_occurred = true;
                    remaining_body_rows = self.body_rows.drain(i..).collect();
                    return Ok(());
                }
                if let Err(e) = row.layout(
                    child_ctx,
                    &mut occupancy,
                    row_idx_offset + i,
                    self.style.table.border_spacing,
                ) {
                    log::warn!("Skipping table row that failed to lay out: {}", e);
                }
            }
            content_height_on_this_page = child_ctx.cursor.1;
            Ok(())
        })?;

        if split_occurred {
            draw_background_and_borders(
                ctx.elements,
                ctx.bounds,
                &self.style,
                block_start_y_in_ctx,
                content_height_on_this_page,
            );

            ctx.cursor.1 = content_start_y_in_ctx
                + content_height_on_this_page
                + self.style.box_model.padding.bottom
                + border_bottom;

            let mut remainder = create_remainder_table(self, remaining_body_rows);
            remainder.measure(&LayoutEnvironment{ engine: ctx.engine, local_page_index: ctx.local_page_index }, BoxConstraints::tight_width(ctx.bounds.width));
            return Ok(LayoutResult::Partial(RenderNode::Table(remainder)));
        }

        draw_background_and_borders(
            ctx.elements,
            ctx.bounds,
            &self.style,
            block_start_y_in_ctx,
            content_height_on_this_page,
        );
        ctx.cursor.1 = content_start_y_in_ctx
            + content_height_on_this_page
            + self.style.box_model.padding.bottom
            + border_bottom;
        ctx.last_v_margin = self.style.box_model.margin.bottom;

        Ok(LayoutResult::Full)
    }
}

fn create_remainder_table(
    original_table: &TableNode,
    remaining_body_rows: Vec<TableRowNode>,
) -> TableNode {
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
    fn new(row: &TableRow, style: &Arc<ComputedStyle>, engine: &LayoutEngine) -> Result<Self, LayoutError> {
        let cells = row
            .cells
            .iter()
            .map(|c| TableCellNode::new(c, style, engine))
            .collect::<Result<Vec<_>,_>>()?;
        Ok(Self {
            cells,
            height: 0.0,
            col_widths: vec![],
        })
    }

    fn measure(&mut self, env: &LayoutEnvironment, col_widths: &[f32]) {
        self.col_widths = col_widths.to_vec();
        let mut max_height: f32 = 0.0;
        let mut col_cursor = 0;
        for cell in self.cells.iter_mut() {
            if col_cursor >= col_widths.len() {
                break;
            }
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
        ctx: &mut LayoutContext,
        occupancy: &mut [Vec<bool>],
        row_idx: usize,
        border_spacing: f32,
    ) -> Result<LayoutResult, LayoutError> {
        let mut current_x = 0.0;
        let mut col_idx = 0;
        for cell in self.cells.iter_mut() {
            while col_idx < self.col_widths.len()
                && occupancy
                .get(row_idx)
                .and_then(|r| r.get(col_idx))
                .map_or(false, |&o| o)
            {
                current_x += self.col_widths[col_idx] + border_spacing;
                col_idx += 1;
            }
            if col_idx >= self.col_widths.len() {
                break;
            }

            let cell_col_idx = col_idx;
            let cell_width = (0..cell.colspan)
                .map(|i| {
                    self.col_widths
                        .get(cell_col_idx + i)
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum::<f32>()
                + (cell.colspan.saturating_sub(1) as f32 * border_spacing);

            let cell_bounds = geom::Rect {
                x: ctx.bounds.x + current_x,
                y: ctx.bounds.y + ctx.cursor.1,
                width: cell_width,
                height: self.height,
            };

            ctx.with_child_bounds(cell_bounds, |cell_ctx| {
                cell.layout(cell_ctx)
            })?;

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

#[derive(Debug, Clone)]
struct TableCellNode {
    content: BlockNode,
    height: f32,
    preferred_width: f32,
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
            height: 0.0,
            preferred_width: 0.0,
            colspan: cell.col_span.max(1),
            rowspan: cell.row_span.max(1),
        })
    }

    fn measure(&mut self, env: &LayoutEnvironment, available_width: f32) {
        let constraint_fixed = BoxConstraints::tight_width(available_width);
        self.height = self.content.measure(env, constraint_fixed).height;

        let constraint_infinite = BoxConstraints {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        };
        self.preferred_width = self.content.measure(env, constraint_infinite).width;
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutContext,
    ) -> Result<LayoutResult, LayoutError> {
        self.content.layout(ctx)
    }
}

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
    let table_width_is_finite = table_width.is_finite();

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

    let mut preferred_widths: Vec<f32> = vec![0.0; columns.len()];
    let infinite_constraint = BoxConstraints {
        min_width: 0.0,
        max_width: f32::INFINITY,
        min_height: 0.0,
        max_height: f32::INFINITY,
    };

    for row in header_rows.iter_mut().chain(body_rows.iter_mut()) {
        let mut col_cursor = 0;
        for cell in &mut row.cells {
            if col_cursor >= columns.len() {
                break;
            }
            if auto_indices.contains(&col_cursor) {
                let preferred = cell.content.measure(env, infinite_constraint).width;
                if cell.colspan == 1 {
                    preferred_widths[col_cursor] = preferred_widths[col_cursor].max(preferred);
                }
            }
            col_cursor += cell.colspan;
        }
    }

    let total_preferred: f32 = auto_indices.iter().map(|&i| preferred_widths[i]).sum();

    if !table_width_is_finite {
        for &i in &auto_indices {
            widths[i] = preferred_widths[i];
        }
        return widths;
    }

    if total_preferred > 0.0 && remaining_width > total_preferred {
        let extra_space = remaining_width - total_preferred;
        for &i in &auto_indices {
            widths[i] = preferred_widths[i] + extra_space * (preferred_widths[i] / total_preferred);
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