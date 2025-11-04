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
use super::style::{self, ComputedStyle};
use super::{IRNode, PipelineError, PositionedElement};
use crate::core::idf::{InlineNode, NodeMetadata};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::core::style::text::TextAlign;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// The main layout engine. It is responsible for orchestrating the multi-pass
/// layout algorithm on a single `IRNode` tree.
#[derive(Clone)]
pub struct LayoutEngine {
    pub(crate) font_manager: Arc<FontManager>,
}

#[derive(Clone)]
struct HeadingInfo {
    id: String,
    level: u8,
    children: Vec<InlineNode>,
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
        let mut processed_nodes = ir_nodes;

        // --- Pre-processing Pass: TOC Generation ---
        // 1. Collect all headings with IDs from the document.
        let mut headings = Vec::new();
        for node in &processed_nodes {
            collect_headings_recursive(node, &mut headings);
        }

        // 2. If any headings were found, traverse the tree again and replace
        //    any `TableOfContents` nodes with generated content.
        if !headings.is_empty() {
            for node in &mut processed_nodes {
                transform_tocs_recursive(node, &headings);
            }
        }

        // --- Main Layout Pass ---
        let mut pages = Vec::new();
        let mut current_work: Option<Box<dyn LayoutNode>> =
            Some(self.build_layout_node_tree(&IRNode::Root(processed_nodes), self.get_default_style()));

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
            IRNode::TableOfContents { .. } => {
                // This node type should have been transformed into a Block node before the layout
                // tree construction phase. If we encounter it here, it's a logic error.
                panic!(
                    "Encountered an IRNode::TableOfContents during layout tree construction. \
                    It should have been pre-processed into a Block."
                );
            }
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

/// Recursively traverses the IR tree to find all headings with an `id` attribute.
fn collect_headings_recursive(node: &IRNode, headings: &mut Vec<HeadingInfo>) {
    if let IRNode::Heading { meta, level, children } = node {
        if let Some(id) = &meta.id {
            headings.push(HeadingInfo {
                id: id.clone(),
                level: *level,
                children: children.clone(),
            });
        }
    }

    match node {
        IRNode::Root(children)
        | IRNode::Block { children, .. }
        | IRNode::FlexContainer { children, .. }
        | IRNode::List { children, .. }
        | IRNode::ListItem { children, .. } => {
            for child in children {
                collect_headings_recursive(child, headings);
            }
        }
        IRNode::Table { header, body, .. } => {
            if let Some(header) = header {
                for row in &header.rows {
                    for cell in &row.cells {
                        for child in &cell.children {
                            collect_headings_recursive(child, headings);
                        }
                    }
                }
            }
            for row in &body.rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        collect_headings_recursive(child, headings);
                    }
                }
            }
        }
        _ => {} // Leaf nodes
    }
}

/// Recursively traverses the IR tree, replacing any `TableOfContents` nodes
/// with a generated `Block` node containing the TOC entries.
fn transform_tocs_recursive(node: &mut IRNode, headings: &[HeadingInfo]) {
    if let IRNode::TableOfContents { meta } = node {
        let toc_entries: Vec<IRNode> = headings
            .iter()
            .map(|h| {
                // Each TOC entry is a flex container to align the title and page number.
                let title_block = IRNode::Paragraph {
                    meta: NodeMetadata {
                        style_override: Some(ElementStyle {
                            flex_grow: Some(1.0),
                            flex_shrink: Some(1.0),
                            text_align: Some(TextAlign::Left),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    children: vec![InlineNode::Hyperlink {
                        href: format!("#{}", h.id),
                        meta: Default::default(),
                        children: h.children.clone(),
                    }],
                };
                let pagenum_block = IRNode::Paragraph {
                    meta: NodeMetadata {
                        style_override: Some(ElementStyle {
                            flex_shrink: Some(0.0),
                            padding: Some(crate::core::style::dimension::Margins {
                                left: 10.0,
                                ..Default::default()
                            }),
                            text_align: Some(TextAlign::Right),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    // The PageReference node itself will be made a link to its target page
                    // during the atomization phase. We no longer need to wrap it in a Hyperlink here.
                    children: vec![crate::core::idf::InlineNode::PageReference {
                        target_id: h.id.clone(),
                        meta: Default::default(),
                        children: vec![],
                    }],
                };

                IRNode::FlexContainer {
                    meta: Default::default(),
                    children: vec![title_block, pagenum_block],
                }
            })
            .collect();

        // Replace the TOC node with the generated block.
        *node = IRNode::Block {
            meta: meta.clone(),
            children: toc_entries,
        };
        return; // Don't recurse into the newly generated content.
    }

    // Recurse into container nodes.
    match node {
        IRNode::Root(children)
        | IRNode::Block { children, .. }
        | IRNode::FlexContainer { children, .. }
        | IRNode::List { children, .. }
        | IRNode::ListItem { children, .. } => {
            for child in children {
                transform_tocs_recursive(child, headings);
            }
        }
        IRNode::Table { header, body, .. } => {
            if let Some(header) = header.as_mut() {
                for row in &mut header.rows {
                    for cell in &mut row.cells {
                        for child in &mut cell.children {
                            transform_tocs_recursive(child, headings);
                        }
                    }
                }
            }
            for row in &mut body.rows {
                for cell in &mut row.cells {
                    for child in &mut cell.children {
                        transform_tocs_recursive(child, headings);
                    }
                }
            }
        }
        _ => {} // Leaf nodes
    }
}