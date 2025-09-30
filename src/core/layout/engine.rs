use super::fonts::FontManager;
use super::page::PageIterator;
use super::style::{self, ComputedStyle};
use super::table;
use super::{block, flex, image, text, IRNode, LayoutBox, PipelineError};
use crate::core::idf::LayoutUnit;
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use std::sync::Arc;

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
    /// It performs all layout passes and returns a stateful `PageIterator`
    /// that will paginate the final geometry tree.
    pub fn paginate_tree<'a>(
        &'a self,
        mut layout_unit: LayoutUnit,
    ) -> Result<PageIterator<'a>, PipelineError> {
        // Pass 1: Measurement & Annotation (e.g., table columns)
        self.measurement_pass(&mut layout_unit.tree)?;

        // Pass 2: Build Geometry Tree (Sizing and Relative Positioning)
        let (page_width, page_height) = style::get_page_dimensions(&self.stylesheet);
        let margins = &self.stylesheet.page.margins;
        let available_width = page_width - margins.left - margins.right;
        let available_height = page_height - margins.top - margins.bottom;

        let layout_tree = self.build_layout_tree(
            &mut layout_unit.tree,
            self.get_default_style(),
            (available_width, available_height),
        );

        // Pass 3: Pagination
        Ok(PageIterator::new(layout_tree, self))
    }

    /// **Pass 1: Measurement & Annotation**
    /// This pass walks the entire `IRNode` tree for a `sequence`, calculating
    /// size-dependent properties and annotating the tree with them.
    pub(crate) fn measurement_pass(&self, node: &mut IRNode) -> Result<(), PipelineError> {
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

    /// **Pass 2: Build LayoutBox Tree (Sizing & Relative Positioning)**
    /// This is a recursive, top-down pass that consumes the `IRNode` tree and produces
    /// a geometry-aware `LayoutBox` tree where every element has a computed size and
    /// a position relative to its parent.
    pub(crate) fn build_layout_tree(
        &self,
        node: &mut IRNode,
        parent_style: Arc<ComputedStyle>,
        available_size: (f32, f32),
    ) -> LayoutBox {
        let style = self.compute_style(node.style_sets(), node.style_override(), &parent_style);

        // Resolve this node's own width based on available space.
        let width = match &style.width {
            Some(Dimension::Pt(w)) => *w,
            Some(Dimension::Percent(p)) => available_size.0 * (p / 100.0),
            // `Auto` or `None` means the width is determined by the container (block) or content (flex/inline).
            // For block, it fills available space. For others, it's determined by the specific layout logic.
            _ => available_size.0,
        };

        // Note: Height resolution must happen *after* children are laid out for `auto` height.
        // We pass the parent's available height down for percentage calculations.
        let child_available_width = width - style.padding.left - style.padding.right;
        let child_available_height = available_size.1 - style.padding.top - style.padding.bottom;
        let child_available_size = (child_available_width, child_available_height);

        // Dispatch to the appropriate layout function to get the box's CONTENT and CONTENT_HEIGHT.
        let mut layout_box = match node {
            IRNode::Root(..) | IRNode::Block { .. } => {
                block::layout_block(self, node, style.clone(), child_available_size)
            }
            IRNode::List { .. } => {
                block::layout_list(self, node, style.clone(), child_available_size)
            }
            IRNode::ListItem { .. } => {
                // This path is for standalone ListItems. Inside a List, they are handled by layout_list.
                // We pass an index of 0 as it's an unknown context.
                block::layout_list_item(self, node, style.clone(), child_available_size, 0)
            }
            IRNode::Paragraph { .. } => {
                text::layout_paragraph(self, node, style.clone(), child_available_size)
            }
            IRNode::Image { .. } => image::layout_image(node, style.clone(), child_available_size),
            IRNode::FlexContainer { .. } => {
                flex::layout_flex_container(self, node, style.clone(), child_available_size)
            }
            IRNode::Table { .. } => table::layout_table(self, node, style.clone(), child_available_size),
        };

        // The height of the content area.
        let content_height = layout_box.rect.height;

        // Finalize height calculation based on the style.
        let final_height = match &style.height {
            Some(Dimension::Pt(h)) => content_height.max(*h),
            Some(Dimension::Percent(p)) => available_size.1 * (p / 100.0),
            _ => content_height, // Auto height
        };

        // The final box for this node has its own position (margins) and size (including padding and margins).
        layout_box.rect.x = style.margin.left;
        layout_box.rect.y = style.margin.top;
        layout_box.rect.width = width;
        layout_box.rect.height = final_height
            + style.padding.top
            + style.padding.bottom
            + style.margin.top
            + style.margin.bottom;

        layout_box
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
        self.font_manager.measure_text(text, style)
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
        // Default page top margin is 10.
        // Root block has 0 margin/padding.
        // Paragraph has 0 margin/padding.
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