// src/core/layout/engine.rs

use super::fonts::{self, LocalFontContext, SharedFontLibrary};
use super::geom::{self, BoxConstraints};
use super::node::{
    AnchorLocation, IndexEntry, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult,
    NodeState, RenderNode,
};
use super::node_kind::NodeKind;
use super::perf::PerformanceTracker;
use super::style::{self, ComputedStyle};
use super::PositionedElement;
use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::builder::NodeRegistry;
use crate::core::layout::LayoutError;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use bumpalo::Bump;
use cosmic_text::{Buffer, Metrics};
use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::nodes::{
    block::BlockBuilder, flex::FlexBuilder, heading::HeadingBuilder, image::ImageBuilder,
    index_marker::IndexMarkerBuilder, list::ListBuilder, list_item::ListItemBuilder,
    page_break::PageBreakBuilder, paragraph::ParagraphBuilder, table::TableBuilder,
};

pub struct PageOutput {
    pub elements: Vec<PositionedElement>,
    pub anchors: HashMap<TextStr, AnchorLocation>,
    pub index_entries: HashMap<TextStr, Vec<IndexEntry>>,
    pub page_number: usize,
}

pub struct LayoutEngine {
    // Thread-local, mutable access to fonts without locking
    font_context: RefCell<LocalFontContext>,
    // Thread-local performance metrics
    metrics: RefCell<PerformanceTracker>,
    registry: Arc<NodeRegistry>,
}

impl LayoutEngine {
    pub fn new(library: &SharedFontLibrary) -> Self {
        let mut registry = NodeRegistry::new();
        registry.register(NodeKind::Root, Box::new(BlockBuilder));
        registry.register(NodeKind::Block, Box::new(BlockBuilder));
        registry.register(NodeKind::Paragraph, Box::new(ParagraphBuilder));
        registry.register(NodeKind::Heading, Box::new(HeadingBuilder));
        registry.register(NodeKind::Image, Box::new(ImageBuilder));
        registry.register(NodeKind::FlexContainer, Box::new(FlexBuilder));
        registry.register(NodeKind::List, Box::new(ListBuilder));
        registry.register(NodeKind::ListItem, Box::new(ListItemBuilder));
        registry.register(NodeKind::Table, Box::new(TableBuilder));
        registry.register(NodeKind::PageBreak, Box::new(PageBreakBuilder));
        registry.register(NodeKind::IndexMarker, Box::new(IndexMarkerBuilder));

        LayoutEngine {
            font_context: RefCell::new(LocalFontContext::new(library)),
            metrics: RefCell::new(PerformanceTracker::default()),
            registry: Arc::new(registry),
        }
    }

    /// Helper to get mutable access to the font system via RefCell.
    pub fn font_system(&self) -> RefMut<'_, cosmic_text::FontSystem> {
        RefMut::map(self.font_context.borrow_mut(), |ctx| &mut ctx.system)
    }

    /// Records a performance metric. Uses interior mutability.
    pub fn record_perf(&self, key: &str, duration: Duration) {
        self.metrics.borrow_mut().record(key, duration);
    }

    /// Dumps statistics to logs and resets them.
    pub fn dump_stats(&self, sequence_id: usize) {
        let metrics = self.metrics.borrow();
        metrics.log_summary(sequence_id);
    }

    pub fn reset_stats(&self) {
        self.metrics.borrow_mut().reset();
    }

    pub fn attrs_from_style<'a>(&self, style: &'a ComputedStyle) -> cosmic_text::Attrs<'a> {
        fonts::attrs_from_style(style)
    }

    pub fn build_render_tree<'a>(
        &self,
        ir_root: &IRNode,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let start = Instant::now();
        let default_style = self.get_default_style();
        let res = self.build_layout_node_tree(ir_root, default_style, arena);
        let duration = start.elapsed();
        self.record_perf("LayoutEngine::build_render_tree", duration);
        res
    }

    pub fn paginate<'a>(
        &'a self,
        stylesheet: &'a Stylesheet,
        root_node: RenderNode<'a>,
        arena: &'a Bump,
    ) -> Result<impl Iterator<Item = Result<PageOutput, LayoutError>> + 'a, LayoutError> {
        let current_master_name = stylesheet
            .default_page_master_name
            .clone()
            .ok_or_else(|| LayoutError::Generic("No default page master defined".to_string()))?;

        // Reset stats at start of pagination
        self.reset_stats();

        Ok(PaginationIterator {
            engine: self,
            stylesheet,
            root_node,
            arena,
            current_state: None,
            current_master_name: Some(current_master_name),
            page_count: 0,
            layout_cache: HashMap::new(),
            finished: false,
        })
    }

    pub(crate) fn build_layout_node_children<'a>(
        &self,
        ir_children: &[IRNode],
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<Vec<RenderNode<'a>>, LayoutError> {
        let mut nodes = Vec::with_capacity(ir_children.len());
        for child_ir in ir_children {
            nodes.push(self.build_layout_node_tree(child_ir, parent_style.clone(), arena)?);
        }
        Ok(nodes)
    }

    pub(crate) fn build_layout_node_tree<'a>(
        &self,
        node: &IRNode,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let kind = NodeKind::from_ir(node);
        let builder = self
            .registry
            .get(kind)
            .ok_or_else(|| LayoutError::BuilderMismatch("Known Node", kind.as_str()))?;

        builder.build(node, self, parent_style, arena)
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
        let mut system = self.font_system();

        let metrics = Metrics::new(style.text.font_size, style.text.line_height);
        let mut buffer = Buffer::new(&mut *system, metrics);

        let attrs = self.attrs_from_style(style);
        buffer.set_text(&mut *system, text, &attrs, cosmic_text::Shaping::Advanced);

        buffer.shape_until_scroll(&mut *system, false);

        let mut max_w: f32 = 0.0;
        for run in buffer.layout_runs() {
            max_w = max_w.max(run.line_w);
        }
        max_w
    }
}

struct PaginationIterator<'a> {
    engine: &'a LayoutEngine,
    stylesheet: &'a Stylesheet,
    root_node: RenderNode<'a>,
    arena: &'a Bump,
    current_state: Option<NodeState>,
    current_master_name: Option<String>,
    page_count: usize,
    layout_cache: HashMap<u64, Box<dyn Any + Send>>,
    finished: bool,
}

impl<'a> Iterator for PaginationIterator<'a> {
    type Item = Result<PageOutput, LayoutError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        const MAX_PAGES: usize = 500;
        self.page_count += 1;

        if self.page_count > MAX_PAGES {
            self.finished = true;
            self.engine.dump_stats(0); // Sequence ID 0 for generic errors/limits
            return Some(Err(LayoutError::Generic(format!(
                "Page limit exceeded ({})",
                MAX_PAGES
            ))));
        }

        self.layout_cache.clear();

        let master_name = match &self.current_master_name {
            Some(n) => n,
            None => {
                self.finished = true;
                return Some(Err(LayoutError::Generic("No page master defined".into())));
            }
        };

        let page_layout = match self.stylesheet.page_masters.get(master_name) {
            Some(l) => l,
            None => {
                self.finished = true;
                return Some(Err(LayoutError::Generic(format!(
                    "Page master '{}' not found",
                    master_name
                ))));
            }
        };

        let (page_width, page_height) = page_layout.size.dimensions_pt();
        let margins = page_layout.margins.clone().unwrap_or_default();
        let content_width = page_width - margins.left - margins.right;
        let content_height = page_height - margins.top - margins.bottom;
        let bounds = geom::Rect {
            x: margins.left,
            y: margins.top,
            width: content_width,
            height: content_height,
        };

        let mut page_elements = Vec::new();
        let mut defined_anchors = HashMap::new();
        let mut index_entries = HashMap::new();

        // Construct Environment with Cache
        let env = LayoutEnvironment {
            engine: self.engine,
            local_page_index: self.page_count - 1,
            cache: &mut self.layout_cache,
        };

        // Construct Context
        let mut ctx = LayoutContext::new(
            env,
            bounds,
            self.arena,
            &mut page_elements,
            &mut defined_anchors,
            &mut index_entries,
        );

        let constraints = BoxConstraints::tight_width(content_width);

        // Run layout
        let layout_start = Instant::now();
        let result = self
            .root_node
            .layout(&mut ctx, constraints, self.current_state.take());
        let layout_dur = layout_start.elapsed();

        // Using a key that identifies page number for individual page performance tracking if needed,
        // but for now relying on the cumulative stats.
        self.engine.record_perf("Page Layout Logic", layout_dur);

        match result {
            Ok(LayoutResult::Finished) => {
                self.finished = true;
                // Since we don't know the exact sequence ID here (it's external to layout),
                // we'll just dump. The calling worker usually has the ID, but for internal
                // layout debugging this is sufficient.
                self.engine.dump_stats(0);
                Some(Ok(PageOutput {
                    elements: page_elements,
                    anchors: defined_anchors,
                    index_entries,
                    page_number: self.page_count,
                }))
            }
            Ok(LayoutResult::Break(next_state)) => {
                if let Some(Some(new_master)) = self.root_node.check_for_page_break() {
                    self.current_master_name = Some(new_master.to_string());
                }
                self.current_state = Some(next_state);

                Some(Ok(PageOutput {
                    elements: page_elements,
                    anchors: defined_anchors,
                    index_entries,
                    page_number: self.page_count,
                }))
            }
            Err(e) => {
                self.finished = true;
                self.engine.dump_stats(0);
                Some(Err(e))
            }
        }
    }
}