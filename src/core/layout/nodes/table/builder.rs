use crate::core::idf::IRNode;
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::nodes::RenderNode;
use crate::core::layout::nodes::block::BlockNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::LayoutError;
use std::sync::Arc;
use super::node::{TableNode, TableRowNode, TableCellNode};

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
            Self::build_rows(&h.rows, &style, engine, store)?
        } else {
            Vec::new()
        };

        let body_vec = Self::build_rows(&body.rows, &style, engine, store)?;

        let id = meta.id.as_ref().map(|s| store.alloc_str(s));
        let style_ref = store.cache_style(style);
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

    fn build_rows(
        rows: &[crate::core::idf::TableRow],
        style: &Arc<ComputedStyle>,
        engine: &LayoutEngine,
        store: &'a LayoutStore,
    ) -> Result<Vec<TableRowNode<'a>>, LayoutError> {
        rows.iter()
            .map(|r| TableRowNode::new(r, style, engine, store))
            .collect()
    }
}

impl<'a> TableRowNode<'a> {
    fn new(
        row: &crate::core::idf::TableRow,
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
}