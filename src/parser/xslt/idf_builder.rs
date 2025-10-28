// FILE: src/parser/xslt/idf_builder.rs
//! An implementation of the `OutputBuilder` trait that constructs an IDF `IRNode` tree.

use super::ast::PreparsedStyles;
use super::output::OutputBuilder;
use crate::core::idf::{IRNode, InlineNode, TableBody, TableCell, TableColumnDefinition, TableRow};
use crate::core::style::dimension::Dimension;

/// An `OutputBuilder` that creates a `Vec<IRNode>`.
pub struct IdfBuilder {
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
}

impl IdfBuilder {
    pub fn new() -> Self {
        Self {
            node_stack: vec![IRNode::Root(Vec::with_capacity(16))],
            inline_stack: vec![],
        }
    }

    /// Consumes the builder and returns the final tree.
    pub fn get_result(mut self) -> Vec<IRNode> {
        if self.node_stack.len() == 1 {
            if let Some(IRNode::Root(children)) = self.node_stack.pop() {
                return children;
            }
        }
        // Fallback for an empty or malformed document
        vec![]
    }

    fn push_block_to_parent(&mut self, node: IRNode) {
        if let Some(parent) = self.node_stack.last_mut() {
            match parent {
                IRNode::Root(c)
                | IRNode::Block { children: c, .. }
                | IRNode::FlexContainer { children: c, .. }
                | IRNode::List { children: c, .. }
                | IRNode::ListItem { children: c, .. } => c.push(node),
                // FIX: Correctly handle adding block content (like a <p>) into a table cell.
                // The parent on the node_stack is the Table, not the cell itself.
                IRNode::Table { body, .. } => {
                    if let Some(last_row) = body.rows.last_mut() {
                        if let Some(last_cell) = last_row.cells.last_mut() {
                            last_cell.children.push(node);
                        } else {
                            // This case can happen if a block is placed inside a <row> but not a <cell>.
                            log::warn!("Attempted to add block content to a table row with no cells.");
                        }
                    } else {
                        // This case can happen if a block is placed inside a <table> but not a <row>.
                        log::warn!("Attempted to add block content to a table with no rows.");
                    }
                }
                _ => log::warn!("Cannot add block node to current parent: {:?}", parent),
            }
        }
    }

    fn push_inline_to_parent(&mut self, node: InlineNode) {
        if let Some(parent_inline) = self.inline_stack.last_mut() {
            if let InlineNode::StyledSpan { children: c, .. }
            | InlineNode::Hyperlink { children: c, .. } = parent_inline
            {
                c.push(node);
                return;
            }
        }

        if let Some(parent_block) = self.node_stack.last_mut() {
            match parent_block {
                IRNode::Paragraph { children: c, .. } => {
                    c.push(node);
                    return;
                },
                // FIX: If the current block context is a Table, auto-wrap the text
                // into the last available cell, creating a paragraph if needed.
                IRNode::Table { body, .. } => {
                    if let Some(cell) = body.rows.last_mut().and_then(|r| r.cells.last_mut()) {
                        // Find or create a paragraph in the cell to hold the inline content
                        if let Some(IRNode::Paragraph { children: p_children, ..}) = cell.children.last_mut() {
                            p_children.push(node);
                        } else {
                            cell.children.push(IRNode::Paragraph {
                                style_sets: vec![],
                                style_override: None,
                                children: vec![node]
                            });
                        }
                        return;
                    }
                },
                _ => {} // Fall through to auto-wrapping logic
            }
        }

        // Auto-wrap loose inline content in a paragraph
        self.push_block_to_parent(IRNode::Paragraph {
            style_sets: vec![],
            style_override: None,
            children: vec![node],
        });
    }
}

impl OutputBuilder for IdfBuilder {
    fn start_block(&mut self, styles: &PreparsedStyles) {
        let node = IRNode::Block {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            children: vec![],
        };
        self.node_stack.push(node);
    }
    fn end_block(&mut self) {
        if self.node_stack.len() > 1 {
            if let Some(node) = self.node_stack.pop() {
                self.push_block_to_parent(node);
            }
        }
    }

    fn start_flex_container(&mut self, styles: &PreparsedStyles) {
        let node = IRNode::FlexContainer {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            children: vec![],
        };
        self.node_stack.push(node);
    }
    fn end_flex_container(&mut self) {
        self.end_block(); // Same logic as block
    }

    fn start_paragraph(&mut self, styles: &PreparsedStyles) {
        let node = IRNode::Paragraph {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            children: vec![],
        };
        self.node_stack.push(node);
    }
    fn end_paragraph(&mut self) {
        self.end_block(); // Same logic as block
    }

    fn start_list(&mut self, styles: &PreparsedStyles) {
        let node = IRNode::List {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            start: None,
            children: vec![],
        };
        self.node_stack.push(node);
    }
    fn end_list(&mut self) {
        self.end_block();
    }

    fn start_list_item(&mut self, styles: &PreparsedStyles) {
        let node = IRNode::ListItem {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            children: vec![],
        };
        self.node_stack.push(node);
    }
    fn end_list_item(&mut self) {
        self.end_block();
    }

    fn start_image(&mut self, styles: &PreparsedStyles) {
        let node = IRNode::Image {
            src: "".to_string(),
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
        };
        self.node_stack.push(node);
    }
    fn end_image(&mut self) {
        self.end_block();
    }

    // --- Table Implementation ---
    fn start_table(&mut self, styles: &PreparsedStyles) {
        let node = IRNode::Table {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            columns: vec![],
            header: None,
            body: Box::new(TableBody::default()),
        };
        self.node_stack.push(node);
    }
    fn end_table(&mut self) {
        self.end_block();
    }

    fn set_table_columns(&mut self, columns: &[Dimension]) {
        if let Some(IRNode::Table { columns: table_cols, .. }) = self.node_stack.last_mut() {
            *table_cols = columns
                .iter()
                .map(|dim| TableColumnDefinition {
                    width: Some(dim.clone()),
                    ..Default::default()
                })
                .collect();
        }
    }

    fn start_table_row(&mut self, _styles: &PreparsedStyles) {
        if let Some(IRNode::Table { body, .. }) = self.node_stack.last_mut() {
            body.rows.push(TableRow { cells: vec![] });
        }
    }
    fn end_table_row(&mut self) {
        // No-op: Rows are managed by the parent table.
    }

    fn start_table_cell(&mut self, styles: &PreparsedStyles) {
        if let Some(IRNode::Table { body, .. }) = self.node_stack.last_mut() {
            if let Some(last_row) = body.rows.last_mut() {
                last_row.cells.push(TableCell {
                    style_sets: styles.style_sets.clone(),
                    style_override: styles.style_override.clone(),
                    children: vec![],
                    ..Default::default()
                });
            }
        }
    }
    fn end_table_cell(&mut self) {
        // No-op: Cells are managed by the parent row.
    }

    fn add_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.push_inline_to_parent(InlineNode::Text(text.to_string()));
        }
    }

    fn start_styled_span(&mut self, styles: &PreparsedStyles) {
        let node = InlineNode::StyledSpan {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            children: vec![],
        };
        self.inline_stack.push(node);
    }
    fn end_styled_span(&mut self) {
        if let Some(node) = self.inline_stack.pop() {
            self.push_inline_to_parent(node);
        }
    }

    fn start_hyperlink(&mut self, styles: &PreparsedStyles) {
        let node = InlineNode::Hyperlink {
            href: "".to_string(),
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            children: vec![],
        };
        self.inline_stack.push(node);
    }
    fn end_hyperlink(&mut self) {
        self.end_styled_span(); // Same logic as span
    }

    fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some(inline_parent) = self.inline_stack.last_mut() {
            if let InlineNode::Hyperlink { href, .. } = inline_parent {
                if name == "href" {
                    *href = value.to_string();
                    return;
                }
            }
        }
        if let Some(block_parent) = self.node_stack.last_mut() {
            if let IRNode::Image { src, .. } = block_parent {
                if name == "src" {
                    *src = value.to_string();
                    return;
                }
            }
        }
        log::warn!("Cannot set attribute '{}' on current builder state.", name);
    }
}