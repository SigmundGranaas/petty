use super::fonts::FontManager;
use super::geom;
use super::node::{AnchorLocation, LayoutContext, LayoutNode, LayoutResult};
use super::nodes::block::BlockNode;
use super::nodes::flex::FlexNode;
use super::nodes::heading::HeadingNode;
use super::nodes::image::ImageNode;
use super::nodes::list::ListNode;
use super::nodes::page_break::PageBreakNode;
use super::nodes::paragraph::ParagraphNode;
use super::nodes::table::TableNode;
use super::nodes::toc::TableOfContentsNode;
use super::style::{self, ComputedStyle};
use super::{IRNode, PipelineError, PositionedElement};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use std::cell::RefCell;
use std::collections::HashMap;
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
    ) -> Result<(Vec<Vec<PositionedElement>>, HashMap<String, AnchorLocation>), PipelineError>
    {
        let mut pages = Vec::new();
        let mut current_work: Option<Box<dyn LayoutNode>> =
            Some(self.build_layout_node_tree(&IRNode::Root(ir_nodes), self.get_default_style()));

        let mut current_master_name = stylesheet
            .default_page_master_name
            .clone()
            .ok_or_else(|| PipelineError::Layout("No default page master defined".to_string()))?;

        // This map will be populated during the layout pass.
        let defined_anchors = RefCell::new(HashMap::<String, AnchorLocation>::new());

        while let Some(mut work_item) = current_work.take() {
            let page_layout = stylesheet.page_masters.get(&current_master_name).ok_or_else(|| {
                PipelineError::Layout(format!("Page master '{}' not found in stylesheet", current_master_name))
            })?;

            let (page_width, page_height) = page_layout.size.dimensions_pt();
            let margins = page_layout.margins.clone().unwrap_or_default();
            let content_width = page_width - margins.left - margins.right;
            let content_height = page_height - margins.top - margins.bottom;

            let page_elements_cell = RefCell::new(Vec::new());
            let bounds = geom::Rect {
                x: margins.left,
                y: margins.top,
                width: content_width,
                height: content_height,
            };

            // Before layout, perform the measurement pass on the current work item.
            work_item.measure(self, content_width);

            let mut ctx = LayoutContext::new(self, bounds, &page_elements_cell, &defined_anchors);
            ctx.local_page_index = pages.len();

            // Layout this page
            let result = work_item.layout(&mut ctx)?;

            let page_elements = page_elements_cell.into_inner();

            // A page should be added if it's the first one, or if it contains any content.
            // This prevents creating empty pages in the middle of a document when an element
            // that doesn't fit causes a page break without rendering anything.
            if pages.is_empty() || !page_elements.is_empty() {
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
        Ok((pages, defined_anchors.into_inner()))
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
            IRNode::Heading { .. } => Box::new(HeadingNode::new(node, self, parent_style)),
            IRNode::TableOfContents { .. } => Box::new(TableOfContentsNode::new(node, self, parent_style)),
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