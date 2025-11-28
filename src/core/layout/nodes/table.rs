// src/core/layout/nodes/table.rs

use crate::core::idf::{IRNode, TableColumnDefinition, TableRow, TextStr};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, RenderNode,
};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use bumpalo::Bump;
use std::sync::Arc;

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
        _ctx: &mut LayoutContext,
        _constraints: BoxConstraints,
        _break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
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
            if col_cursor >= col_widths.len() { break; }
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