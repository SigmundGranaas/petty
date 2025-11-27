// src/core/layout/engine.rs

use super::geom::{self, BoxConstraints};
use super::node::{
    AnchorLocation, IndexEntry, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult,
    NodeState, RenderNode,
};
use super::nodes::block::BlockNode;
use super::nodes::flex::FlexNode;
use super::nodes::heading::HeadingNode;
use super::nodes::image::ImageNode;
use super::nodes::index_marker::IndexMarkerNode;
use super::nodes::list::ListNode;
use super::nodes::list_item::ListItemNode;
use super::nodes::page_break::PageBreakNode;
use super::nodes::paragraph::ParagraphNode;
use super::nodes::table::TableNode;
use super::style::{self, ComputedStyle};
use super::{FontManager, PipelineError, PositionedElement};
use crate::core::idf::IRNode;
use crate::core::layout::LayoutError;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use cosmic_text::{Buffer, Metrics};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct LayoutEngine {
    pub(crate) font_manager: Arc<FontManager>,
}

impl LayoutEngine {
    pub fn new(font_manager: Arc<FontManager>) -> Self {
        LayoutEngine { font_manager }
    }

    pub fn paginate(
        &self,
        stylesheet: &Stylesheet,
        ir_nodes: Vec<IRNode>,
    ) -> Result<
        (
            Vec<Vec<PositionedElement>>,
            HashMap<String, AnchorLocation>,
            HashMap<String, Vec<IndexEntry>>,
        ),
        PipelineError,
    > {
        let mut pages = Vec::new();

        let root_node =
            self.build_layout_node_tree(&IRNode::Root(ir_nodes), self.get_default_style())?;

        let mut current_master_name = stylesheet.default_page_master_name.clone().ok_or_else(
            || PipelineError::Layout("No default page master defined".to_string()),
        )?;

        let mut defined_anchors = HashMap::<String, AnchorLocation>::new();
        let mut index_entries = HashMap::<String, Vec<IndexEntry>>::new();
        let mut layout_cache = HashMap::<u64, Box<dyn Any + Send>>::new();
        let mut current_state: Option<NodeState> = None;

        let mut page_count = 0;
        const MAX_PAGES: usize = 200;

        loop {
            page_count += 1;
            if page_count > MAX_PAGES {
                return Err(PipelineError::Layout(format!(
                    "Document exceeded maximum page limit ({}). Possible infinite layout loop.",
                    MAX_PAGES
                )));
            }

            let page_layout =
                stylesheet
                    .page_masters
                    .get(&current_master_name)
                    .ok_or_else(|| {
                        PipelineError::Layout(format!(
                            "Page master '{}' not found in stylesheet",
                            current_master_name
                        ))
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

            let env = LayoutEnvironment {
                engine: self,
                local_page_index: pages.len(),
            };

            let constraints = BoxConstraints::tight_width(content_width);

            // Construct Context with new API
            let mut ctx = LayoutContext::new(
                env,
                bounds,
                &mut page_elements,
                &mut defined_anchors,
                &mut index_entries,
                &mut layout_cache,
            );

            let result = root_node
                .layout(&mut ctx, constraints, current_state.take())
                .map_err(|e| PipelineError::Layout(e.to_string()))?;

            pages.push(page_elements);

            match result {
                LayoutResult::Finished => {
                    break;
                }
                LayoutResult::Break(next_state) => {
                    if let Some(Some(new_master)) = root_node.check_for_page_break() {
                        current_master_name = new_master;
                    }
                    current_state = Some(next_state);
                }
            }
        }

        Ok((pages, defined_anchors, index_entries))
    }

    pub(crate) fn build_layout_node_children(
        &self,
        ir_children: &[IRNode],
        parent_style: Arc<ComputedStyle>,
    ) -> Result<Vec<RenderNode>, LayoutError> {
        ir_children
            .iter()
            .map(|child_ir| self.build_layout_node_tree(child_ir, parent_style.clone()))
            .collect()
    }

    /// Constructs the appropriate `RenderNode` variant based on the `IRNode`.
    pub(crate) fn build_layout_node_tree(
        &self,
        node: &IRNode,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        match node {
            IRNode::Root(_) => BlockNode::build(node, self, parent_style),
            IRNode::Block { .. } => BlockNode::build(node, self, parent_style),
            IRNode::ListItem { .. } => BlockNode::build(node, self, parent_style),
            IRNode::Paragraph { .. } => ParagraphNode::build(node, self, parent_style),
            IRNode::Heading { .. } => HeadingNode::build(node, self, parent_style),
            IRNode::Image { .. } => ImageNode::build(node, self, parent_style),
            IRNode::FlexContainer { .. } => FlexNode::build(node, self, parent_style),
            IRNode::List { .. } => ListNode::build(node, self, parent_style),
            IRNode::Table { .. } => TableNode::build(node, self, parent_style),
            IRNode::PageBreak { .. } => PageBreakNode::build(node, self, parent_style),
            IRNode::IndexMarker { .. } => IndexMarkerNode::build(node, self, parent_style),
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
        let mut system = self.font_manager.system.lock().unwrap();

        // No caching: create fresh buffer
        let metrics = Metrics::new(style.text.font_size, style.text.line_height);
        let mut buffer = Buffer::new(&mut system, metrics);

        let attrs = self.font_manager.attrs_from_style(style);
        buffer.set_text(&mut system, text, &attrs, cosmic_text::Shaping::Advanced);

        buffer.shape_until_scroll(&mut system, false);

        let mut max_w: f32 = 0.0;
        for run in buffer.layout_runs() {
            max_w = max_w.max(run.line_w);
        }
        max_w
    }
}