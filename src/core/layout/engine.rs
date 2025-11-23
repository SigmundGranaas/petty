use super::geom::{self, BoxConstraints};
use super::node::{AnchorLocation, IndexEntry, LayoutBuffer, LayoutEnvironment, LayoutNode, LayoutResult};
use super::nodes::block::{BlockBuilder, RootBuilder};
use super::nodes::flex::FlexBuilder;
use super::nodes::heading::HeadingBuilder;
use super::nodes::image::ImageBuilder;
use super::nodes::index_marker::IndexMarkerBuilder;
use super::nodes::list::ListBuilder;
use super::nodes::page_break::PageBreakBuilder;
use super::nodes::paragraph::ParagraphBuilder;
use super::nodes::table::TableBuilder;
use super::style::{self, ComputedStyle};
use super::{FontManager, IRNode, PipelineError, PositionedElement};
use crate::core::layout::builder::NodeRegistry;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use cosmic_text::{Buffer, Metrics};
use std::collections::HashMap;
use std::sync::Arc;

/// The main layout engine. It is responsible for orchestrating the multi-pass
/// layout algorithm on a single `IRNode` tree.
#[derive(Clone)]
pub struct LayoutEngine {
    pub(crate) font_manager: Arc<FontManager>,
    pub(crate) registry: Arc<NodeRegistry>,
}

impl LayoutEngine {
    /// Creates a new layout engine.
    pub fn new(font_manager: Arc<FontManager>) -> Self {
        let mut registry = NodeRegistry::new();

        registry.register("root", Box::new(RootBuilder));
        registry.register("block", Box::new(BlockBuilder));
        registry.register("list-item", Box::new(BlockBuilder)); // List items use block logic
        registry.register("paragraph", Box::new(ParagraphBuilder));
        registry.register("heading", Box::new(HeadingBuilder));
        registry.register("image", Box::new(ImageBuilder));
        registry.register("flex-container", Box::new(FlexBuilder));
        registry.register("list", Box::new(ListBuilder));
        registry.register("table", Box::new(TableBuilder));
        registry.register("page-break", Box::new(PageBreakBuilder));
        registry.register("index-marker", Box::new(IndexMarkerBuilder));

        LayoutEngine {
            font_manager,
            registry: Arc::new(registry),
        }
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
        let mut current_work: Option<Box<dyn LayoutNode>> = Some(self.build_layout_node_tree(
            &IRNode::Root(ir_nodes),
            self.get_default_style(),
        ));

        let mut current_master_name = stylesheet
            .default_page_master_name
            .clone()
            .ok_or_else(|| PipelineError::Layout("No default page master defined".to_string()))?;

        let mut defined_anchors = HashMap::<String, AnchorLocation>::new();
        let mut index_entries = HashMap::<String, Vec<IndexEntry>>::new();

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

            let env = LayoutEnvironment {
                engine: self,
                local_page_index: pages.len(),
            };

            let constraints = BoxConstraints::tight_width(content_width);
            work_item.measure(&env, constraints);

            let mut buf = LayoutBuffer::new(
                bounds,
                &mut page_elements,
                &mut defined_anchors,
                &mut index_entries,
            );

            let result = work_item.layout(&env, &mut buf)?;

            // Always push the page, even if empty. This ensures that structural page breaks
            // or empty containers that span pages are respected.
            pages.push(page_elements);

            match result {
                LayoutResult::Full => {}
                LayoutResult::Partial(mut remainder) => {
                    if let Some(new_master) = remainder.check_for_page_break() {
                        current_master_name = new_master.unwrap_or(current_master_name);
                    }
                    current_work = Some(remainder);
                }
            }
        }
        Ok((pages, defined_anchors, index_entries))
    }

    pub(crate) fn build_layout_node_children(
        &self,
        ir_children: &[IRNode],
        parent_style: Arc<ComputedStyle>,
    ) -> Vec<Box<dyn LayoutNode>> {
        ir_children
            .iter()
            .map(|child_ir| self.build_layout_node_tree(child_ir, parent_style.clone()))
            .collect()
    }

    pub(crate) fn build_layout_node_tree(
        &self,
        node: &IRNode,
        parent_style: Arc<ComputedStyle>,
    ) -> Box<dyn LayoutNode> {
        let kind = node.kind();
        if let Some(builder) = self.registry.get(kind) {
            builder.build(node, self, parent_style)
        } else {
            panic!("No NodeBuilder registered for node type: {}", kind);
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

    /// Measures text width using `cosmic-text`.
    /// Used by Table/Flex logic for intrinsic sizing.
    pub fn measure_text_width(&self, text: &str, style: &Arc<ComputedStyle>) -> f32 {
        let mut system = self.font_manager.system.lock().unwrap();
        let metrics = Metrics::new(style.font_size, style.line_height);
        let mut buffer = Buffer::new(&mut system, metrics);

        let attrs = self.font_manager.attrs_from_style(style);
        // Pass &attrs as required by cosmic-text API
        buffer.set_text(&mut system, text, &attrs, cosmic_text::Shaping::Advanced);

        // No wrapping for width measurement implies infinite line length
        buffer.set_size(&mut system, None, None);
        buffer.shape_until_scroll(&mut system, false);

        let mut max_w: f32 = 0.0;
        for run in buffer.layout_runs() {
            max_w = max_w.max(run.line_w);
        }
        max_w
    }
}