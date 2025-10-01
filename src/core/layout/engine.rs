// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/engine.rs
// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/engine.rs
use super::fonts::FontManager;
use super::geom;
use super::node::{LayoutContext, LayoutNode, LayoutResult};
use super::nodes::block::BlockNode;
use super::nodes::flex::FlexNode;
use super::nodes::image::ImageNode;
use super::nodes::list::ListNode;
use super::nodes::page_break::PageBreakNode;
use super::nodes::paragraph::ParagraphNode;
use super::nodes::table::TableNode;
use super::style::{self, ComputedStyle};
use super::{IRNode, PipelineError, PositionedElement};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use std::sync::Arc;

/// The main layout engine. It is responsible for orchestrating the multi-pass
/// layout algorithm on a single `IRNode` tree.
#[derive(Clone)]
pub struct LayoutEngine {
    pub(crate) font_manager: Arc<FontManager>,
}

impl LayoutEngine {
    /// Creates a new layout engine.
    pub fn new(font_manager: Arc<FontManager>) -> Self {
        LayoutEngine { font_manager }
    }

    /// The main entry point into the layout process.
    /// This method implements a cooperative pagination algorithm that processes a
    /// complete `IRNode` tree, breaking it into pages based on content flow and
    /// explicit page breaks.
    pub fn paginate(
        &self,
        stylesheet: &Stylesheet,
        ir_nodes: Vec<IRNode>,
    ) -> Result<Vec<Vec<PositionedElement>>, PipelineError> {
        let mut pages = Vec::new();
        let mut current_work: Option<Box<dyn LayoutNode>> =
            Some(self.build_layout_node_tree(&IRNode::Root(ir_nodes), self.get_default_style()));

        let mut current_master_name = stylesheet
            .default_page_master_name
            .clone()
            .ok_or_else(|| PipelineError::Layout("No default page master defined".to_string()))?;

        while let Some(mut work_item) = current_work.take() {
            let page_layout = stylesheet.page_masters.get(&current_master_name).ok_or_else(|| {
                PipelineError::Layout(format!("Page master '{}' not found in stylesheet", current_master_name))
            })?;

            let (page_width, page_height) = page_layout.size.dimensions_pt();
            let margins = page_layout.margins.clone().unwrap_or_default();
            let content_width = page_width - margins.left - margins.right;
            let content_height = page_height - margins.top - margins.bottom;

            let mut page_elements = Vec::new();
            let bounds = geom::Rect {
                x: margins.left,
                y: margins.top,
                width: content_width,
                height: content_height,
            };

            // Before layout, perform the measurement pass on the current work item.
            work_item.measure(self, content_width);

            let mut ctx = LayoutContext::new(self, bounds, &mut page_elements);

            // Layout this page
            let result = work_item.layout(&mut ctx)?;

            if !page_elements.is_empty() || pages.is_empty() {
                pages.push(page_elements);
            }

            // Prepare for next page
            match result {
                LayoutResult::Full => { /* Done with all content, loop will terminate. */ }
                LayoutResult::Partial(mut remainder) => {
                    // Check if the reason for the page break was an explicit <page-break> tag.
                    if let Some(new_master) = remainder.check_for_page_break() {
                        current_master_name = new_master.unwrap_or(current_master_name);
                    }
                    current_work = Some(remainder);
                }
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
            IRNode::PageBreak { master_name } => Box::new(PageBreakNode::new(master_name.clone())),
            // ListItem is handled internally by ListNode
            IRNode::ListItem { .. } => {
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
    use crate::core::style::stylesheet::PageLayout;

    fn create_test_engine() -> LayoutEngine {
        let mut font_manager = FontManager::new();
        font_manager.load_fallback_font().unwrap();
        LayoutEngine::new(Arc::new(font_manager))
    }

    #[test]
    fn test_paginate_simple_paragraph() {
        let engine = create_test_engine();
        let mut stylesheet = Stylesheet::default();
        stylesheet.page_masters.insert("master".to_string(), PageLayout::default());
        stylesheet.default_page_master_name = Some("master".to_string());

        let ir_nodes = vec![IRNode::Paragraph {
            style_sets: vec![],
            style_override: None,
            children: vec![InlineNode::Text("Hello World".to_string())],
        }];

        let mut pages = engine.paginate(&stylesheet, ir_nodes).unwrap();
        let page1 = pages.remove(0);

        assert!(!page1.is_empty(), "Page should have elements");
        let text_element = &page1[0];

        let default_margin = stylesheet.page_masters["master"].margins.as_ref().map_or(0.0, |m| m.top);
        assert!((text_element.y - default_margin).abs() < 0.1);
        // Default font size 12, line height 14.4
        assert_eq!(text_element.height, 14.4);
    }

    #[test]
    fn test_block_with_margin_and_padding() {
        let engine = create_test_engine();
        let mut stylesheet = Stylesheet::default();
        stylesheet.page_masters.insert(
            "master".to_string(),
            PageLayout {
                margins: Some(Margins::default()),
                ..Default::default()
            },
        );
        stylesheet.default_page_master_name = Some("master".to_string());

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
        let ir_nodes = vec![IRNode::Block {
            style_sets: vec![],
            style_override: Some(block_style_override),
            children: vec![IRNode::Paragraph {
                style_sets: vec![],
                style_override: None,
                children: vec![InlineNode::Text("Inside".to_string())],
            }],
        }];

        let mut pages = engine.paginate(&stylesheet, ir_nodes).unwrap();
        let page1 = pages.remove(0);

        assert_eq!(page1.len(), 1, "Should have one text element");
        let text_el = &page1[0];
        let page_margin = stylesheet.page_masters["master"].margins.as_ref().map_or(0.0, |m| m.top);

        // y = page_margin_top(0) + block_margin_top(20) + block_padding_top(10) = 30.0
        assert!((text_el.y - (page_margin + 20.0 + 10.0)).abs() < 0.1);
    }
}