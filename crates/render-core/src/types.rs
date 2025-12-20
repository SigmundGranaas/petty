use petty_types::{ApiIndexEntry, TocEntry};
use std::collections::HashMap;

// Re-export LaidOutSequence from petty-layout
pub use petty_layout::LaidOutSequence;

#[derive(Debug, Clone)]
pub struct ResolvedAnchor {
    pub global_page_index: usize,
    pub y_pos: f32,
}

#[derive(Debug, Clone)]
pub struct HyperlinkLocation {
    pub global_page_index: usize,
    pub rect: [f32; 4],
    pub target_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct Pass1Result {
    pub resolved_anchors: HashMap<String, ResolvedAnchor>,
    pub toc_entries: Vec<TocEntry>,
    pub total_pages: usize,
    pub hyperlink_locations: Vec<HyperlinkLocation>,
    pub index_entries: Vec<ApiIndexEntry>,
}
