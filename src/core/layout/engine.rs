// src/core/layout/engine.rs

use super::fonts::{SharedFontLibrary, FontData};
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
use crate::core::layout::config::LayoutConfig;
use crate::core::layout::LayoutError;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::core::style::font::{FontWeight, FontStyle};
use bumpalo::Bump;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};
use crate::core::layout::nodes::paragraph_utils::ShapedRun;

// Added imports for Hashing
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::nodes::{
    block::BlockBuilder, flex::FlexBuilder, heading::HeadingBuilder, image::ImageBuilder,
    index_marker::IndexMarkerBuilder, list::ListBuilder, list_item::ListItemBuilder,
    page_break::PageBreakBuilder, paragraph::ParagraphBuilder, table::TableBuilder,
};

pub struct LayoutStore {
    pub bump: Bump,
    style_cache: RefCell<HashMap<ComputedStyle, Arc<ComputedStyle>>>,
}

impl LayoutStore {
    pub fn new() -> Self {
        Self {
            bump: Bump::new(),
            style_cache: RefCell::new(HashMap::with_capacity(512)),
        }
    }

    pub fn arena(&self) -> &Bump {
        &self.bump
    }

    pub fn alloc_str(&self, s: &str) -> &str {
        self.bump.alloc_str(s)
    }

    pub fn cache_style(&self, style: Arc<ComputedStyle>) -> &ComputedStyle {
        let mut cache = self.style_cache.borrow_mut();
        if let Some(existing) = cache.get(&style) {
            unsafe { &*(Arc::as_ptr(existing)) }
        } else {
            let key = (*style).clone();
            cache.insert(key, style.clone());
            unsafe { &*(Arc::as_ptr(&style)) }
        }
    }

    pub fn canonicalize_style(&self, style: Arc<ComputedStyle>) -> Arc<ComputedStyle> {
        let mut cache = self.style_cache.borrow_mut();
        if let Some(existing) = cache.get(&style) {
            existing.clone()
        } else {
            cache.insert((*style).clone(), style.clone());
            style
        }
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

#[derive(Hash, PartialEq, Eq, Clone)]
struct FontCacheKey {
    family: Arc<String>,
    weight: u16,
    style: u8,
}

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct ShapingCacheKey {
    pub text: String,
    pub style: Arc<ComputedStyle>,
}

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct MultiSpanCacheKey {
    // Vector of (Text, StyleHash)
    pub spans: Vec<(String, u64)>,
}

// Thread-local caches
thread_local! {
    static LOCAL_FONT_CACHE: RefCell<HashMap<FontCacheKey, Option<FontData>>> = RefCell::new(HashMap::new());
    static LOCAL_SHAPING_CACHE: RefCell<HashMap<ShapingCacheKey, Arc<Vec<ShapedRun>>>> = RefCell::new(HashMap::new());
    static LOCAL_MULTI_SPAN_CACHE: RefCell<HashMap<MultiSpanCacheKey, Arc<Vec<ShapedRun>>>> = RefCell::new(HashMap::new());
}

pub struct LayoutEngine {
    pub font_library: SharedFontLibrary,
    metrics: Mutex<PerformanceTracker>,
    registry: Arc<NodeRegistry>,
    config: LayoutConfig,
    font_cache: RwLock<HashMap<FontCacheKey, Option<FontData>>>,
    pub shaping_cache: RwLock<HashMap<ShapingCacheKey, Arc<Vec<ShapedRun>>>>,
    pub multi_span_cache: RwLock<HashMap<MultiSpanCacheKey, Arc<Vec<ShapedRun>>>>,
    // Add text measurement cache
    text_measure_cache: RwLock<HashMap<(String, u64), f32>>,
}

// Automatically dump statistics when the LayoutEngine is dropped (e.g., when a worker finishes).
impl Drop for LayoutEngine {
    fn drop(&mut self) {
        self.dump_stats(0);
    }
}

impl LayoutEngine {
    pub fn new(library: &SharedFontLibrary, config: LayoutConfig) -> Self {
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
            font_library: library.clone(),
            metrics: Mutex::new(PerformanceTracker::default()),
            registry: Arc::new(registry),
            config,
            font_cache: RwLock::new(HashMap::new()),
            shaping_cache: RwLock::new(HashMap::new()),
            multi_span_cache: RwLock::new(HashMap::new()),
            text_measure_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn font_db(&self) -> Arc<RwLock<fontdb::Database>> {
        self.font_library.db.clone()
    }

    pub fn config(&self) -> LayoutConfig {
        self.config
    }

    pub fn record_perf(&self, key: &str, duration: Duration) {
        if let Ok(mut m) = self.metrics.lock() {
            m.record(key, duration);
        }
    }

    pub fn count_hit(&self) {
        if let Ok(m) = self.metrics.lock() {
            m.count_hit();
        }
    }

    pub fn count_miss(&self) {
        if let Ok(m) = self.metrics.lock() {
            m.count_miss();
        }
    }

    pub fn dump_stats(&self, sequence_id: usize) {
        if let Ok(m) = self.metrics.lock() {
            m.log_summary(sequence_id);
        }
    }

    pub fn reset_stats(&self) {
        if let Ok(mut m) = self.metrics.lock() {
            m.reset();
        }
    }

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

        if let Some(data) = LOCAL_FONT_CACHE.with(|c| c.borrow().get(&key).cloned()) {
            return data;
        }

        if let Ok(cache) = self.font_cache.read() {
            if let Some(cached_result) = cache.get(&key) {
                LOCAL_FONT_CACHE.with(|local| {
                    local.borrow_mut().insert(key.clone(), cached_result.clone());
                });
                return cached_result.clone();
            }
        }

        let font_data = self.font_library.resolve_font_data(style);
        if let Ok(mut cache) = self.font_cache.write() {
            cache.insert(key.clone(), font_data.clone());
        }
        LOCAL_FONT_CACHE.with(|local| {
            local.borrow_mut().insert(key, font_data.clone());
        });

        font_data
    }

    pub fn get_cached_shaping_run(&self, key: &ShapingCacheKey) -> Option<Arc<Vec<ShapedRun>>> {
        if let Some(run) = LOCAL_SHAPING_CACHE.with(|c| c.borrow().get(key).cloned()) {
            return Some(run);
        }
        if let Ok(cache) = self.shaping_cache.read() {
            if let Some(run) = cache.get(key) {
                LOCAL_SHAPING_CACHE.with(|local| {
                    local.borrow_mut().insert(key.clone(), run.clone());
                });
                return Some(run.clone());
            }
        }
        None
    }

    pub fn cache_shaping_run(&self, key: ShapingCacheKey, runs: Arc<Vec<ShapedRun>>) {
        LOCAL_SHAPING_CACHE.with(|local| {
            local.borrow_mut().insert(key.clone(), runs.clone());
        });
        if let Ok(mut cache) = self.shaping_cache.write() {
            cache.insert(key, runs);
        }
    }

    pub fn get_cached_multi_span_run(&self, key: &MultiSpanCacheKey) -> Option<Arc<Vec<ShapedRun>>> {
        if let Some(run) = LOCAL_MULTI_SPAN_CACHE.with(|c| c.borrow().get(key).cloned()) {
            return Some(run);
        }
        if let Ok(cache) = self.multi_span_cache.read() {
            if let Some(run) = cache.get(key) {
                LOCAL_MULTI_SPAN_CACHE.with(|local| {
                    local.borrow_mut().insert(key.clone(), run.clone());
                });
                return Some(run.clone());
            }
        }
        None
    }

    pub fn cache_multi_span_run(&self, key: MultiSpanCacheKey, runs: Arc<Vec<ShapedRun>>) {
        LOCAL_MULTI_SPAN_CACHE.with(|local| {
            local.borrow_mut().insert(key.clone(), runs.clone());
        });
        if let Ok(mut cache) = self.multi_span_cache.write() {
            cache.insert(key, runs);
        }
    }

    pub fn build_render_tree<'a>(
        &self,
        ir_root: &IRNode,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let start = Instant::now();
        let default_style = self.get_default_style();
        let res = self.build_layout_node_tree(ir_root, default_style, store);
        let duration = start.elapsed();
        self.record_perf("LayoutEngine::build_render_tree", duration);
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

        // Removed self.reset_stats() to allow accumulation of metrics across sequences
        // for average calculations (e.g. time per sequence).

        Ok(PaginationIterator {
            engine: self,
            stylesheet,
            root_node,
            arena: &store.bump,
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
        store: &'a LayoutStore,
    ) -> Result<Vec<RenderNode<'a>>, LayoutError> {
        let mut nodes = Vec::with_capacity(ir_children.len());
        for child_ir in ir_children {
            nodes.push(self.build_layout_node_tree(child_ir, parent_style.clone(), store)?);
        }
        Ok(nodes)
    }

    pub(crate) fn build_layout_node_tree<'a>(
        &self,
        node: &IRNode,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let kind = NodeKind::from_ir(node);
        let builder = self
            .registry
            .get(kind)
            .ok_or_else(|| LayoutError::BuilderMismatch("Known Node", kind.as_str()))?;

        builder.build(node, self, parent_style, store)
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

    pub fn measure_text_width(&self, text: &str, style: &ComputedStyle) -> f32 {
        // OPTIMIZATION: Check text measurement cache first
        let mut hasher = DefaultHasher::new();
        style.hash(&mut hasher);
        let style_hash = hasher.finish();

        let key = (text.to_string(), style_hash);

        if let Ok(cache) = self.text_measure_cache.read() {
            if let Some(&width) = cache.get(&key) {
                return width;
            }
        }

        let font_data = match self.get_font_for_style(style) {
            Some(d) => d,
            None => return 0.0,
        };

        let face = font_data.face();

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();

        let glyph_buffer = rustybuzz::shape(face, &[], buffer);
        let positions = glyph_buffer.glyph_positions();

        let scale = style.text.font_size / face.units_per_em() as f32;

        let width: f32 = positions.iter().map(|p| p.x_advance as f32 * scale).sum();

        // Update cache
        if let Ok(mut cache) = self.text_measure_cache.write() {
            cache.insert(key, width);
        }

        width
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
        if self.finished { return None; }

        let start = Instant::now();

        // Loop block used to allow `break` for returning values, enabling measurement
        // of the entire logic block including error paths.
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
                cache: &mut self.layout_cache,
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

        // Record total time taken for this page's generation logic
        self.engine.record_perf("PageLayout::generate_page", start.elapsed());

        result
    }
}