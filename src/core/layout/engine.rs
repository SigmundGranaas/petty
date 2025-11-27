use super::geom::{self, BoxConstraints, Size};
use super::node::{AnchorLocation, IndexEntry, LayoutContext, LayoutEnvironment, LayoutResult, RenderNode, LayoutNode};
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
use crate::core::layout::LayoutError;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use cosmic_text::{Buffer, Metrics};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Holds aggregated statistics for a specific node type.
#[derive(Debug, Default)]
pub struct ProfilerStats {
    pub measure_count: AtomicU64,
    pub measure_micros: AtomicU64,
    pub layout_count: AtomicU64,
    pub layout_micros: AtomicU64,
}

/// A thread-safe profiler registry.
#[derive(Debug)]
pub struct LayoutProfiler {
    stats: Mutex<HashMap<String, Arc<ProfilerStats>>>,
}

impl LayoutProfiler {
    fn new() -> Self {
        Self {
            stats: Mutex::new(HashMap::new()),
        }
    }

    fn get_stats(&self, kind: &str) -> Arc<ProfilerStats> {
        let mut map = self.stats.lock().unwrap();
        map.entry(kind.to_string())
            .or_insert_with(|| Arc::new(ProfilerStats::default()))
            .clone()
    }

    fn print_report(&self) {
        let map = self.stats.lock().unwrap();
        let mut entries: Vec<_> = map.iter().collect();

        entries.sort_by_key(|(_, stats)| {
            let total = stats.measure_micros.load(Ordering::Relaxed) + stats.layout_micros.load(Ordering::Relaxed);
            std::cmp::Reverse(total)
        });

        println!("\n=== Layout Engine Performance Profile (Inclusive) ===");
        println!("{:<20} | {:<10} | {:<12} | {:<10} | {:<12}",
                 "Node Type", "Measure #", "Measure (ms)", "Layout #", "Layout (ms)");
        println!("{:-<75}", "");

        for (kind, stats) in entries {
            let m_count = stats.measure_count.load(Ordering::Relaxed);
            let m_time_us = stats.measure_micros.load(Ordering::Relaxed);
            let l_count = stats.layout_count.load(Ordering::Relaxed);
            let l_time_us = stats.layout_micros.load(Ordering::Relaxed);

            println!("{:<20} | {:<10} | {:<12.2} | {:<10} | {:<12.2}",
                     kind,
                     m_count,
                     m_time_us as f64 / 1000.0,
                     l_count,
                     l_time_us as f64 / 1000.0
            );
        }
        println!("===================================================\n");
    }
}

#[derive(Debug)]
struct ProfiledNode {
    inner: Box<dyn LayoutNode>,
    stats: Arc<ProfilerStats>,
}

impl LayoutNode for ProfiledNode {
    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let start = Instant::now();
        let result = self.inner.measure(env, constraints);
        let elapsed = start.elapsed().as_micros() as u64;

        self.stats.measure_count.fetch_add(1, Ordering::Relaxed);
        self.stats.measure_micros.fetch_add(elapsed, Ordering::Relaxed);

        result
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>,
    ) -> Result<LayoutResult, LayoutError> {
        let start = Instant::now();
        let result = self.inner.layout(ctx, constraints, break_state);
        let elapsed = start.elapsed().as_micros() as u64;

        self.stats.layout_count.fetch_add(1, Ordering::Relaxed);
        self.stats.layout_micros.fetch_add(elapsed, Ordering::Relaxed);

        result
    }

    fn style(&self) -> &Arc<ComputedStyle> {
        self.inner.style()
    }

    fn check_for_page_break(&self) -> Option<Option<String>> {
        self.inner.check_for_page_break()
    }
}

#[derive(Clone)]
pub struct LayoutEngine {
    pub(crate) font_manager: Arc<FontManager>,
    pub(crate) registry: Arc<NodeRegistry>,
    pub(crate) profiler: Arc<LayoutProfiler>,
    pub(crate) scratch_buffer: Arc<Mutex<Option<Buffer>>>,
}

impl LayoutEngine {
    pub fn new(font_manager: Arc<FontManager>) -> Self {
        let mut registry = NodeRegistry::new();

        registry.register("root", Box::new(RootBuilder));
        registry.register("block", Box::new(BlockBuilder));
        registry.register("list-item", Box::new(BlockBuilder));
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
            profiler: Arc::new(LayoutProfiler::new()),
            scratch_buffer: Arc::new(Mutex::new(None)),
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

        let root_node = self.build_layout_node_tree(
            &IRNode::Root(ir_nodes),
            self.get_default_style(),
        )?;

        let mut current_master_name = stylesheet
            .default_page_master_name
            .clone()
            .ok_or_else(|| PipelineError::Layout("No default page master defined".to_string()))?;

        let mut defined_anchors = HashMap::<String, AnchorLocation>::new();
        let mut index_entries = HashMap::<String, Vec<IndexEntry>>::new();
        let mut current_state: Option<Box<dyn Any + Send>> = None;

        let mut page_count = 0;
        const MAX_PAGES: usize = 200;

        loop {
            page_count += 1;
            if page_count > MAX_PAGES {
                panic!("Layout Engine Panic: Exceeded {} pages.", MAX_PAGES);
            }

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

            let mut ctx = LayoutContext::new(
                env,
                bounds,
                &mut page_elements,
                &mut defined_anchors,
                &mut index_entries,
            );

            let result = root_node.layout(&mut ctx, constraints, current_state.take())
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

        self.profiler.print_report();

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

    pub(crate) fn build_layout_node_tree(
        &self,
        node: &IRNode,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        let kind = node.kind();
        let inner_node = if let Some(builder) = self.registry.get(kind) {
            builder.build(node, self, parent_style)
        } else {
            Err(LayoutError::Generic(format!("No NodeBuilder registered for node type: {}", kind)))
        }?;

        Ok(Box::new(ProfiledNode {
            inner: inner_node,
            stats: self.profiler.get_stats(kind),
        }))
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

        let mut guard = self.scratch_buffer.lock().unwrap();

        if guard.is_none() {
            let metrics = Metrics::new(style.text.font_size, style.text.line_height);
            *guard = Some(Buffer::new(&mut system, metrics));
        }

        let buffer = guard.as_mut().unwrap();

        // Reset metrics for current style
        let metrics = Metrics::new(style.text.font_size, style.text.line_height);
        buffer.set_metrics_and_size(&mut system, metrics, None, None);

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