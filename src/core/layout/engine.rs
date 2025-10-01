// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/engine.rs
use super::fonts::FontManager;
use super::geom;
use super::node::{LayoutContext, LayoutNode, LayoutResult};
use super::nodes::block::BlockNode;
use super::nodes::flex::FlexNode;
use super::nodes::image::ImageNode;
use super::nodes::list::ListNode;
use super::nodes::paragraph::ParagraphNode;
use super::nodes::table::TableNode;
use super::style::{self, ComputedStyle};
use super::{IRNode, LayoutError, PipelineError, PositionedElement};
use crate::core::idf::LayoutUnit;
use crate::core::style::dimension::Margins;
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
    /// This is the new, cooperative pagination implementation.
    pub fn paginate_tree(
        &self,
        layout_unit: LayoutUnit,
    ) -> Result<Vec<Vec<PositionedElement>>, PipelineError> {
        let (page_width, page_height) = style::get_page_dimensions(&self.stylesheet);
        let default_margins = Margins::default();
        let margins = self.stylesheet.page.margins.as_ref().unwrap_or(&default_margins);
        let content_width = page_width - margins.left - margins.right;
        let content_height = page_height - margins.top - margins.bottom;

        // 1. Build the full LayoutNode tree from the IRNode tree.
        let mut root_node = self.build_layout_node_tree(&layout_unit.tree, self.get_default_style());

        // 2. Perform the measurement pass on the LayoutNode tree.
        root_node.measure(self, content_width);

        // 3. Start the pagination loop.
        let mut work_item: Option<Box<dyn LayoutNode>> = Some(root_node);
        let mut pages = Vec::new();

        while let Some(mut current_node) = work_item.take() {
            let mut page_elements = Vec::new();
            let bounds = geom::Rect {
                x: margins.left,
                y: margins.top,
                width: content_width,
                height: content_height,
            };
            let mut ctx = LayoutContext::new(self, bounds, &mut page_elements);

            match current_node.layout(&mut ctx) {
                Ok(LayoutResult::Full) => {
                    // Document finished, loop will terminate.
                }
                Ok(LayoutResult::Partial(remainder)) => {
                    work_item = Some(remainder);
                }
                Err(e @ LayoutError::ElementTooLarge(..)) => {
                    log::error!("Layout error: {}. Skipping offending element.", e);
                    // The element is skipped, but we continue processing the rest of the document
                    // by taking the remainder if one was produced, or just continuing if not.
                    // In a simple case (like an image), there's no remainder.
                }
            }

            if !page_elements.is_empty() || pages.is_empty() {
                pages.push(page_elements);
            }
        }

        Ok(pages)
    }

    /// Factory function to convert an `IRNode` into a `LayoutNode`.
    pub(crate) fn build_layout_node_tree(
        &self,
        node: &IRNode,
        parent_style: Arc<ComputedStyle>,
    ) -> Box<dyn LayoutNode> {
        match node {
            IRNode::Root(_) => Box::new(BlockNode::new_root(node, self, parent_style)),
            IRNode::Block { .. } => Box::new(BlockNode::new(node, self, parent_style)),
            IRNode::List { .. } => Box::new(ListNode::new(node, self, parent_style)),
            IRNode::FlexContainer { .. } => Box::new(FlexNode::new(node, self, parent_style)),
            IRNode::Table { .. } => Box::new(TableNode::new(node, self, parent_style)),
            IRNode::Paragraph { .. } => Box::new(ParagraphNode::new(node, self, parent_style)),
            IRNode::Image { .. } => Box::new(ImageNode::new(node, self, parent_style)),
            // ListItem is handled internally by ListNode
            IRNode::ListItem { .. } => {
                // This case should ideally not be hit if ListItems are always in Lists.
                // We'll treat it as a Block for robustness.
                log::warn!("Orphan ListItem found; treating as a Block.");
                Box::new(BlockNode::new(node, self, parent_style))
            }
        }
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
    use crate::core::idf::InlineNode;
    use crate::core::style::dimension::Margins;
    use serde_json::Value;
    use std::sync::Arc;

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

        let mut pages = engine.paginate_tree(layout_unit).unwrap();
        let page1 = pages.remove(0);

        assert!(!page1.is_empty(), "Page should have elements");
        let text_element = &page1[0];

        let default_margin = engine.stylesheet.page.margins.as_ref().map_or(0.0, |m| m.top);
        assert!((text_element.y - default_margin).abs() < 0.1);
        // Default font size 12, line height 14.4
        assert_eq!(text_element.height, 14.4);
    }

    #[test]
    fn test_block_with_margin_and_padding() {
        let engine = create_test_engine();
        let block_style_override = ElementStyle {
            margin: Some(Margins {
                top: 20.0,
                bottom: 20.0,
                ..Default::default()
            }),
            padding: Some(Margins {
                top: 10.0,
                bottom: 10.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let tree = IRNode::Root(vec![IRNode::Block {
            style_sets: vec![],
            style_override: Some(block_style_override),
            children: vec![IRNode::Paragraph {
                style_sets: vec![],
                style_override: None,
                children: vec![InlineNode::Text("Inside".to_string())],
            }],
        }]);

        let layout_unit = LayoutUnit {
            tree,
            context: Value::Null.into(),
        };
        let mut pages = engine.paginate_tree(layout_unit).unwrap();
        let page1 = pages.remove(0);

        assert_eq!(page1.len(), 1, "Should have one text element");
        let text_el = &page1[0];
        let page_margin = engine.stylesheet.page.margins.as_ref().map_or(0.0, |m| m.top);

        // y = page_margin_top(0) + block_margin_top(20) + block_padding_top(10) = 30.0
        assert!((text_el.y - (page_margin + 20.0 + 10.0)).abs() < 0.1);
    }
}