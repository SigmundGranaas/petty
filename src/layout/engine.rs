// src/layout/engine.rs

//! The main layout engine struct and entry point.

use super::page::PageIterator;
use super::style::{self, ComputedStyle};
use super::table;
use super::text;
use super::{IRNode, LayoutUnit, PipelineError, Stylesheet};
use crate::stylesheet::ElementStyle;

/// The main layout engine. It is responsible for orchestrating the multi-pass
/// layout algorithm on a single `IRNode` tree.
#[derive(Clone)]
pub struct LayoutEngine {
    pub(crate) stylesheet: Stylesheet,
    // Caches for expensive operations like font measurement would go here.
}

impl LayoutEngine {
    /// Creates a new layout engine with the given stylesheet.
    pub fn new(stylesheet: Stylesheet) -> Self {
        LayoutEngine { stylesheet }
    }

    /// The main entry point into the layout process for a single `sequence`.
    /// It performs the measurement pass and returns a stateful `PageIterator`
    /// that will perform the positioning pass lazily.
    pub fn paginate_tree<'a>(
        &'a self,
        layout_unit: &'a LayoutUnit,
    ) -> Result<PageIterator<'a>, PipelineError> {
        // The measurement pass is a prerequisite for layout.
        let mut annotated_tree = layout_unit.tree.clone();
        self.measurement_pass(&mut annotated_tree)?;
        Ok(PageIterator::new(annotated_tree, self))
    }

    /// **Pass 1: Measurement & Annotation**
    /// This pass walks the entire `IRNode` tree for a `sequence`, calculating
    /// size-dependent properties and annotating the tree with them.
    fn measurement_pass(&self, node: &mut IRNode) -> Result<(), PipelineError> {
        match node {
            IRNode::Table {
                columns,
                calculated_widths,
                header,
                body,
                ..
            } => {
                let (page_width, _) = style::get_page_dimensions(&self.stylesheet);
                let table_width = page_width
                    - self.stylesheet.page.margins.left
                    - self.stylesheet.page.margins.right;
                *calculated_widths = table::calculate_column_widths(columns, table_width);

                if let Some(h) = header {
                    for row in &mut h.rows {
                        for cell in &mut row.cells {
                            for child in &mut cell.children {
                                self.measurement_pass(child)?;
                            }
                        }
                    }
                }
                for row in &mut body.rows {
                    for cell in &mut row.cells {
                        for child in &mut cell.children {
                            self.measurement_pass(child)?;
                        }
                    }
                }
            }
            IRNode::Root(children)
            | IRNode::Block { children, .. }
            | IRNode::FlexContainer { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => {
                for child in children {
                    self.measurement_pass(child)?;
                }
            }
            _ => {} // Other nodes don't need pre-measurement in this version.
        }
        Ok(())
    }

    pub fn compute_style(
        &self,
        style_name: Option<&str>,
        style_override: Option<&ElementStyle>,
        parent_style: &ComputedStyle,
    ) -> ComputedStyle {
        style::compute_style(&self.stylesheet, style_name, style_override, parent_style)
    }

    pub fn get_default_style(&self) -> ComputedStyle {
        style::get_default_style()
    }

    pub fn measure_text_width(&self, text: &str, style: &ComputedStyle) -> f32 {
        text::measure_text_width(self, text, style)
    }
}

// Integration tests can go here, verifying the whole process.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::{InlineNode, IRNode, LayoutUnit};
    use crate::stylesheet::{ElementStyle, Stylesheet};
    use serde_json::Value;
    use std::collections::HashMap;

    fn create_test_engine() -> LayoutEngine {
        let mut styles = HashMap::new();
        styles.insert(
            "h1".to_string(),
            ElementStyle {
                font_size: Some(24.0),
                ..Default::default()
            },
        );
        let stylesheet = Stylesheet {
            styles,
            page: Default::default(),
            templates: HashMap::new(),
            page_sequences: HashMap::new(),
        };
        LayoutEngine::new(stylesheet)
    }

    #[test]
    fn test_paginate_simple_paragraph() {
        let engine = create_test_engine();
        let tree = IRNode::Root(vec![IRNode::Paragraph {
            style_name: None,
            style_override: None,
            children: vec![InlineNode::Text("Hello World".to_string())],
        }]);
        let layout_unit = LayoutUnit {
            tree,
            context: Value::Null,
        };

        let mut page_iter = engine.paginate_tree(&layout_unit).unwrap();
        let page1 = page_iter.next().unwrap();

        assert!(!page1.is_empty(), "Page should have elements");
        let text_element = &page1[0];
        // Default page top margin is now 10 (from PageLayout::default()).
        // Element's own margin/padding is 0.0 (from ComputedStyle::default()).
        // So, the text should start at y=10.0.
        assert_eq!(text_element.y, 10.0);
        // Default top margin is 10
        // Default font size 12, line height 14.4
        assert_eq!(text_element.height, 14.4);
    }
}