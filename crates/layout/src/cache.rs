use crate::style::ComputedStyle;
use crate::text::shaper::ShapedRun;
use crate::fonts::FontData;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::cell::RefCell;
use std::hash::Hash;

// --- Cache Keys ---

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct FontCacheKey {
    pub family: Arc<String>,
    pub weight: u16,
    pub style: u8,
}

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct ShapingCacheKey {
    pub text: String,
    pub style: Arc<ComputedStyle>,
}

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct MultiSpanCacheKey {
    pub spans: Vec<(String, u64)>,
}

// --- The Manager ---

/// Manages all long-lived caches for the layout engine.
pub struct LayoutCache {
    pub fonts: RwLock<HashMap<FontCacheKey, Option<FontData>>>,
    pub shaping: RwLock<HashMap<ShapingCacheKey, Arc<Vec<ShapedRun>>>>,
    pub multi_span: RwLock<HashMap<MultiSpanCacheKey, Arc<Vec<ShapedRun>>>>,
    pub measurements: RwLock<HashMap<(String, u64), f32>>,
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self {
            fonts: RwLock::new(HashMap::new()),
            shaping: RwLock::new(HashMap::new()),
            multi_span: RwLock::new(HashMap::new()),
            measurements: RwLock::new(HashMap::new()),
        }
    }
}

impl LayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&self) {
        if let Ok(mut c) = self.fonts.write() { c.clear(); }
        if let Ok(mut c) = self.shaping.write() { c.clear(); }
        if let Ok(mut c) = self.multi_span.write() { c.clear(); }
        if let Ok(mut c) = self.measurements.write() { c.clear(); }
    }
}

/// Thread-local layer to avoid lock contention on hot paths.
/// Transient data for the current thread/request.
pub struct ThreadLocalCache {
    pub fonts: RefCell<HashMap<FontCacheKey, Option<FontData>>>,
    pub shaping: RefCell<HashMap<ShapingCacheKey, Arc<Vec<ShapedRun>>>>,
    pub multi_span: RefCell<HashMap<MultiSpanCacheKey, Arc<Vec<ShapedRun>>>>,
    // Generic scratch space for node-specific layout calculations (e.g., Paragraph layouts)
    // This is passed to nodes via LayoutEnvironment
    pub node_layouts: RefCell<HashMap<u64, Box<dyn Any + Send>>>,
}

impl Default for ThreadLocalCache {
    fn default() -> Self {
        Self {
            fonts: RefCell::new(HashMap::new()),
            shaping: RefCell::new(HashMap::new()),
            multi_span: RefCell::new(HashMap::new()),
            node_layouts: RefCell::new(HashMap::new()),
        }
    }
}