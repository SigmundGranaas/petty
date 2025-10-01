// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/table.rs
use crate::core::idf::{IRNode, TableColumnDefinition, TableRow};
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{geom, LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::sync::Arc;

// --- Main Table Node ---

#[derive(Debug)]
pub struct TableNode {
    rows: Vec<TableRowNode>,
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

        let mut rows = Vec::new();
        if let Some(h) = header {
            rows.extend(h.rows.iter().map(|r| TableRowNode::new(r, &style, engine)));
        }
        rows.extend(body.rows.iter().map(|r| TableRowNode::new(r, &style, engine)));

        Self {
            rows,
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
        self.calculated_widths = calculate_column_widths(&self.columns, available_width);
        for row in &mut self.rows {
            row.measure(engine, available_width, &self.calculated_widths);
        }
    }

    fn measure_content_height(&mut self, _engine: &LayoutEngine, _available_width: f32) -> f32 {
        self.rows.iter().map(|r| r.height).sum()
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        for (i, row) in self.rows.iter_mut().enumerate() {
            if row.height > ctx.bounds.height {
                // Return an error for the whole table if a single row is too large.
                return Err(LayoutError::ElementTooLarge(row.height, ctx.bounds.height));
            }
            if row.height > ctx.available_height() && !ctx.is_empty() {
                let remaining_rows = self.rows.drain(i..).collect();
                return Ok(LayoutResult::Partial(Box::new(TableNode {
                    rows: remaining_rows,
                    style: self.style.clone(),
                    columns: self.columns.clone(),
                    calculated_widths: self.calculated_widths.clone(),
                })));
            }
            // A row's layout can't fail, but we handle the error just in case.
            if let Err(e) = row.layout(ctx) {
                log::warn!("Skipping table row that failed to lay out: {}", e);
            }
        }
        Ok(LayoutResult::Full)
    }
}

// --- Table Row Node ---

#[derive(Debug)]
pub struct TableRowNode {
    cells: Vec<TableCellNode>,
    height: f32, // Calculated in `measure` pass
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

    fn measure(&mut self, engine: &LayoutEngine, _available_width: f32, col_widths: &[f32]) {
        self.col_widths = col_widths.to_vec();
        let mut max_height: f32 = 0.0;
        for (i, cell) in self.cells.iter_mut().enumerate() {
            let cell_width = col_widths.get(i).copied().unwrap_or(0.0);
            cell.measure(engine, cell_width);
            max_height = max_height.max(cell.height);
        }
        self.height = max_height;
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        let mut current_x = 0.0;
        for (i, cell) in self.cells.iter_mut().enumerate() {
            let cell_width = self.col_widths.get(i).copied().unwrap_or(0.0);
            let cell_bounds = geom::Rect {
                x: ctx.bounds.x + current_x,
                y: ctx.bounds.y + ctx.cursor.1,
                width: cell_width,
                height: self.height, // Use the pre-calculated max height
            };
            let mut cell_ctx = LayoutContext {
                engine: ctx.engine,
                bounds: cell_bounds,
                cursor: (0.0, 0.0),
                elements: unsafe { &mut *(ctx.elements as *mut Vec<PositionedElement>) },
            };
            // Propagate errors from cells, though a cell (Block) shouldn't error.
            cell.layout(&mut cell_ctx)?;
            current_x += cell_width;
        }
        ctx.advance_cursor(self.height);
        Ok(LayoutResult::Full)
    }
}

// --- Table Cell Node ---

#[derive(Debug)]
struct TableCellNode {
    content: BlockNode,
    height: f32, // Calculated during measurement
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
        }
    }

    fn measure(&mut self, engine: &LayoutEngine, available_width: f32) {
        self.height = self.content.measure_content_height(engine, available_width);
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        self.content.layout(ctx)
    }
}

// --- Helper Functions ---

fn calculate_column_widths(columns: &[TableColumnDefinition], table_width: f32) -> Vec<f32> {
    let mut widths = vec![0.0; columns.len()];
    let mut auto_indices = vec![];
    let mut remaining_width = table_width;
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
                _ => auto_indices.push(i),
            }
        } else {
            auto_indices.push(i);
        }
    }
    if !auto_indices.is_empty() && remaining_width > 0.0 {
        let width_per_auto = remaining_width / auto_indices.len() as f32;
        for i in auto_indices {
            widths[i] = width_per_auto;
        }
    }
    widths
}