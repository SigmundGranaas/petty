// FILE: src/layout/engine.rs
// src/layout/engine.rs

use super::fonts::FontManager;
use super::PageIterator;
use super::style::{self, ComputedStyle};
use super::table;
use super::{IRNode, PipelineError};
use std::sync::Arc;
use crate::core::idf::LayoutUnit;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};

/// The main layout engine. It is responsible for orchestrating the multi-pass
/// layout algorithm on a single `IRNode` tree.
#[derive(Clone)]
pub struct LayoutEngine {
    pub(crate) stylesheet: Stylesheet,
    pub(crate) font_manager: Arc<FontManager>,
}

impl LayoutEngine {
    /// Creates a new layout engine with the given stylesheet.
    pub fn new(stylesheet: Stylesheet, font_manager: Arc<FontManager>) -> Self {
        LayoutEngine {
            stylesheet,
            font_manager,
        }
    }

    /// The main entry point into the layout process for a single `sequence`.
    /// It performs the measurement pass and returns a stateful `PageIterator`
    /// that will perform the positioning pass lazily.
    pub fn paginate_tree<'a>(
        &'a self,
        mut layout_unit: LayoutUnit,
    ) -> Result<PageIterator<'a>, PipelineError> {
        // The measurement pass is a prerequisite for layout.
        self.measurement_pass(&mut layout_unit.tree)?;
        Ok(PageIterator::new(layout_unit, self))
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
        style_sets: &[Arc<ElementStyle>],
        style_override: Option<&ElementStyle>,
        parent_style: &Arc<ComputedStyle>,
    ) -> Arc<ComputedStyle> {
        style::compute_style(style_sets, style_override, parent_style)
    }

    pub fn get_default_style(&self) -> Arc<ComputedStyle> {
        style::get_default_style()
    }

    pub fn measure_text_width(&self, text: &str, style: &Arc<ComputedStyle>) -> f32 {
        self.font_manager
            .measure_text(text, &style.font_family, style.font_size)
    }
}

// Integration tests can go here, verifying the whole process.
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::Arc;
    use crate::core::idf::InlineNode;
    use crate::core::style::dimension::Margins;

    fn create_test_engine() -> LayoutEngine {
        let stylesheet = Stylesheet::default();
        let mut font_manager = FontManager::new();
        font_manager.load_fallback_font().unwrap();
        LayoutEngine::new(stylesheet, Arc::new(font_manager))
    }

    #[test]
    fn test_paginate_simple_paragraph() {
        let engine = create_test_engine();
        let tree = IRNode::Root(vec![IRNode::Paragraph {
            style_sets: vec![],
            style_override: None,
            children: vec![InlineNode::Text("Hello World".to_string())],
        }]);
        let layout_unit = LayoutUnit {
            tree,
            context: Value::Null.into(),
        };

        let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
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

    #[test]
    fn test_block_with_margin_and_padding() {
        let engine = create_test_engine();
        let block_style_override = ElementStyle {
            margin: Some(Margins { top: 20.0, bottom: 20.0, ..Default::default() }),
            padding: Some(Margins { top: 10.0, bottom: 10.0, ..Default::default() }),
            ..Default::default()
        };
        let tree = IRNode::Root(vec![
            IRNode::Block {
                style_sets: vec![],
                style_override: Some(block_style_override),
                children: vec![
                    IRNode::Paragraph {
                        style_sets: vec![],
                        style_override: None,
                        children: vec![InlineNode::Text("Inside".to_string())]
                    }
                ],
            }
        ]);

        let layout_unit = LayoutUnit { tree, context: Value::Null.into() };
        let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
        let page1 = page_iter.next().unwrap();

        assert_eq!(page1.len(), 1, "Should have one text element");
        let text_el = &page1[0];

        // y = page_margin_top(10) + block_margin_top(20) + block_padding_top(10) = 40.0
        assert_eq!(text_el.y, 40.0);
    }
}