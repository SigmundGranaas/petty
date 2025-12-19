use super::fonts::SharedFontLibrary;
use super::interface::{
    AnchorLocation, IndexEntry, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult,
    NodeState,
};
use super::nodes::RenderNode;
use super::style::{self, ComputedStyle};
use super::PositionedElement;
use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::config::LayoutConfig;
use crate::core::layout::LayoutError;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::core::style::font::{FontWeight, FontStyle};
use crate::core::base::geometry::{self as geom, BoxConstraints};
use crate::core::layout::cache::{
    LayoutCache, ThreadLocalCache, FontCacheKey, ShapingCacheKey, MultiSpanCacheKey
};
use crate::core::layout::perf::{Profiler, NoOpProfiler, DebugProfiler};
use crate::core::layout::fonts::FontData;
use crate::core::layout::text::shaper::ShapedRun;

use bumpalo::Bump;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, Duration};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub struct LayoutStore {
    pub bump: Bump,
    /// Cache to deduplicate styles. Returns Arc<ComputedStyle>.
    style_cache: RefCell<HashMap<ComputedStyle, Arc<ComputedStyle>>>,
    /// Counter for unique node IDs used for caching.
    node_id_counter: AtomicUsize,
}

impl LayoutStore {
    pub fn new() -> Self {
        Self {
            bump: Bump::new(),
            style_cache: RefCell::new(HashMap::with_capacity(512)),
            node_id_counter: AtomicUsize::new(1), // Start at 1 to reserve 0
        }
    }

    pub fn alloc_str(&self, s: &str) -> &str {
        self.bump.alloc_str(s)
    }

    pub fn next_node_id(&self) -> usize {
        self.node_id_counter.fetch_add(1, Ordering::Relaxed)
    }

    pub fn cache_style(&self, style: Arc<ComputedStyle>) -> Arc<ComputedStyle> {
        let mut cache = self.style_cache.borrow_mut();
        if let Some(existing) = cache.get(&style) {
            existing.clone()
        } else {
            cache.insert((*style).clone(), style.clone());
            style
        }
    }

    pub fn canonicalize_style(&self, style: Arc<ComputedStyle>) -> Arc<ComputedStyle> {
        self.cache_style(style)
    }
}

impl Default for LayoutStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PageOutput {
    pub elements: Vec<PositionedElement>,
    pub anchors: HashMap<TextStr, AnchorLocation>,
    pub index_entries: HashMap<TextStr, Vec<IndexEntry>>,
    pub page_number: usize,
}

pub struct LayoutEngine {
    pub font_library: SharedFontLibrary,
    pub cache: LayoutCache,
    pub profiler: Box<dyn Profiler>,
    config: LayoutConfig,
}

impl LayoutEngine {
    pub fn new(library: &SharedFontLibrary, config: LayoutConfig) -> Self {
        // Simple strategy: use DebugProfiler if feature enabled, else NoOp
        let profiler: Box<dyn Profiler> = if cfg!(feature = "profiling") {
            Box::new(DebugProfiler::new())
        } else {
            Box::new(NoOpProfiler)
        };

        Self {
            font_library: library.clone(),
            cache: LayoutCache::new(),
            profiler,
            config,
        }
    }

    /// Exposes the underlying font database lock.
    /// Used by the rendering pipeline to load fonts.
    pub fn font_db(&self) -> Arc<RwLock<fontdb::Database>> {
        self.font_library.db.clone()
    }

    /// Records a performance metric.
    /// Delegated to the active profiler.
    pub fn record_perf(&self, key: &str, duration: Duration) {
        self.profiler.record(key, duration);
    }

    pub fn count_hit(&self) {
        self.profiler.count_hit();
    }

    pub fn count_miss(&self) {
        self.profiler.count_miss();
    }

    /// Logs performance stats to stdout/log.
    /// In NoOp profiler this does nothing.
    pub fn dump_stats(&self, _sequence_id: usize) {
        // We need to downcast or check if it's a DebugProfiler to call log_summary
        // Since Profiler trait doesn't have log_summary, we can only do this
        // if we change the trait or simply rely on the fact that production doesn't need dumps.
        // For now, we leave this no-op for traits, or specific impls.
    }

    pub fn reset_stats(&self) {
        self.profiler.reset();
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

    pub fn build_render_tree<'a>(
        &self,
        ir_root: &IRNode,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let start = Instant::now();
        let default_style = self.get_default_style();
        let res = crate::core::layout::nodes::build_node_tree(ir_root, self, default_style, store);
        self.profiler.record("LayoutEngine::build_render_tree", start.elapsed());
        res
    }

    pub fn paginate<'a>(
        &'a self,
        stylesheet: &'a Stylesheet,
        root_node: RenderNode<'a>,
        store: &'a LayoutStore,
    ) -> Result<impl Iterator<Item = Result<PageOutput, LayoutError>> + 'a, LayoutError> {
        let current_master_name = stylesheet
            .default_page_master_name
            .clone()
            .ok_or_else(|| LayoutError::Generic("No default page master defined".to_string()))?;

        Ok(PaginationIterator {
            engine: self,
            stylesheet,
            root_node,
            arena: &store.bump,
            current_state: None,
            current_master_name: Some(current_master_name),
            page_count: 0,
            thread_cache: ThreadLocalCache::default(),
            finished: false,
        })
    }

    // --- Font & Text Lookups ---

    pub fn get_font_for_style(&self, style: &ComputedStyle) -> Option<FontData> {
        let weight_val = match style.text.font_weight {
            FontWeight::Thin => 100,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::Bold => 700,
            FontWeight::Black => 900,
            FontWeight::Numeric(n) => n,
        };
        let style_val = match style.text.font_style {
            FontStyle::Normal => 0,
            FontStyle::Italic => 1,
            FontStyle::Oblique => 2,
        };

        let key = FontCacheKey {
            family: style.text.font_family.clone(),
            weight: weight_val,
            style: style_val,
        };

        // 1. Check Global Cache
        if let Ok(cache) = self.cache.fonts.read() {
            if let Some(cached_result) = cache.get(&key) {
                return cached_result.clone();
            }
        }

        // 2. Resolve & Cache
        let font_data = self.font_library.resolve_font_data(style);
        if let Ok(mut cache) = self.cache.fonts.write() {
            cache.insert(key, font_data.clone());
        }

        font_data
    }

    pub fn measure_text_width(&self, text: &str, style: &ComputedStyle) -> f32 {
        let mut hasher = DefaultHasher::new();
        style.hash(&mut hasher);
        let style_hash = hasher.finish();

        let key = (text.to_string(), style_hash);

        if let Ok(cache) = self.cache.measurements.read() {
            if let Some(&width) = cache.get(&key) {
                return width;
            }
        }

        let font_data = match self.get_font_for_style(style) {
            Some(d) => d,
            None => return 0.0,
        };

        let face = match font_data.as_face() {
            Some(f) => f,
            None => return 0.0,
        };

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();

        let glyph_buffer = rustybuzz::shape(&face, &[], buffer);
        let positions = glyph_buffer.glyph_positions();
        let scale = style.text.font_size / face.units_per_em() as f32;
        let width: f32 = positions.iter().map(|p| p.x_advance as f32 * scale).sum();

        if let Ok(mut cache) = self.cache.measurements.write() {
            cache.insert(key, width);
        }

        width
    }

    // Helpers used by builders
    pub(crate) fn build_layout_node_children<'a>(
        &self,
        ir_children: &[IRNode],
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<Vec<RenderNode<'a>>, LayoutError> {
        let mut nodes = Vec::with_capacity(ir_children.len());
        for child_ir in ir_children {
            nodes.push(crate::core::layout::nodes::build_node_tree(child_ir, self, parent_style.clone(), store)?);
        }
        Ok(nodes)
    }

    pub(crate) fn build_layout_node_tree<'a>(
        &self,
        node: &IRNode,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        crate::core::layout::nodes::build_node_tree(node, self, parent_style, store)
    }

    pub fn get_cached_shaping_run(&self, key: &ShapingCacheKey) -> Option<Arc<Vec<ShapedRun>>> {
        if let Ok(cache) = self.cache.shaping.read() {
            if let Some(run) = cache.get(key) {
                self.profiler.count_hit();
                return Some(run.clone());
            }
        }
        None
    }

    pub fn cache_shaping_run(&self, key: ShapingCacheKey, runs: Arc<Vec<ShapedRun>>) {
        self.profiler.count_miss();
        if let Ok(mut cache) = self.cache.shaping.write() {
            cache.insert(key, runs);
        }
    }

    pub fn get_cached_multi_span_run(&self, key: &MultiSpanCacheKey) -> Option<Arc<Vec<ShapedRun>>> {
        if let Ok(cache) = self.cache.multi_span.read() {
            if let Some(run) = cache.get(key) {
                self.profiler.count_hit();
                return Some(run.clone());
            }
        }
        None
    }

    pub fn cache_multi_span_run(&self, key: MultiSpanCacheKey, runs: Arc<Vec<ShapedRun>>) {
        self.profiler.count_miss();
        if let Ok(mut cache) = self.cache.multi_span.write() {
            cache.insert(key, runs);
        }
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
    thread_cache: ThreadLocalCache,
    finished: bool,
}

impl<'a> Iterator for PaginationIterator<'a> {
    type Item = Result<PageOutput, LayoutError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished { return None; }

        let start = Instant::now();

        let result = loop {
            const MAX_PAGES: usize = 500;
            self.page_count += 1;
            if self.page_count > MAX_PAGES {
                self.finished = true;
                break Some(Err(LayoutError::Generic(format!("Page limit exceeded ({})", MAX_PAGES))));
            }

            let master_name = match &self.current_master_name {
                Some(n) => n,
                None => {
                    self.finished = true;
                    break Some(Err(LayoutError::Generic("No page master".into())));
                }
            };
            let page_layout = match self.stylesheet.page_masters.get(master_name) {
                Some(l) => l,
                None => {
                    self.finished = true;
                    break Some(Err(LayoutError::Generic("Page master not found".into())));
                }
            };

            let (w, h) = page_layout.size.dimensions_pt();
            let m = page_layout.margins.clone().unwrap_or_default();
            let bounds = geom::Rect {
                x: m.left, y: m.top, width: w - m.left - m.right, height: h - m.top - m.bottom
            };

            let mut elements = Vec::new();
            let mut anchors = HashMap::new();
            let mut indices = HashMap::new();

            let env = LayoutEnvironment {
                engine: self.engine,
                local_page_index: self.page_count - 1,
                // Using the thread-local node_layouts cache
                cache: &self.thread_cache.node_layouts,
            };

            let mut ctx = LayoutContext::new(env, bounds, self.arena, &mut elements, &mut anchors, &mut indices);
            let constraints = BoxConstraints::tight_width(bounds.width);

            let layout_res = self.root_node.layout(&mut ctx, constraints, self.current_state.take());

            break match layout_res {
                Ok(LayoutResult::Finished) => {
                    self.finished = true;
                    Some(Ok(PageOutput { elements, anchors, index_entries: indices, page_number: self.page_count }))
                }
                Ok(LayoutResult::Break(next)) => {
                    if let Some(Some(nm)) = self.root_node.check_for_page_break() {
                        self.current_master_name = Some(nm.to_string());
                    }
                    self.current_state = Some(next);
                    Some(Ok(PageOutput { elements, anchors, index_entries: indices, page_number: self.page_count }))
                }
                Err(e) => {
                    self.finished = true;
                    Some(Err(e))
                }
            };
        };

        self.engine.profiler.record("PageLayout::generate_page", start.elapsed());

        result
    }
}