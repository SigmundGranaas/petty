use petty_types::{ApiIndexEntry, TocEntry};
use petty_idf::SharedData;
use petty_layout::{AnchorLocation, IndexEntry, PositionedElement, LayoutElement};
use std::collections::HashMap;

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

/// The result of laying out a single document/data item.
///
/// Contains all positioned elements organized by page, along with
/// resources, anchors, and metadata needed for rendering.
pub struct LaidOutSequence {
    /// Pages of positioned elements ready for rendering.
    pub pages: Vec<Vec<PositionedElement>>,
    /// Resources (images, etc.) referenced by the document.
    pub resources: HashMap<String, SharedData>,
    /// Defined anchors and their locations for cross-references.
    pub defined_anchors: HashMap<String, AnchorLocation>,
    /// Table of contents entries extracted during layout.
    pub toc_entries: Vec<TocEntry>,
    /// Index entries grouped by term.
    pub index_entries: HashMap<String, Vec<IndexEntry>>,
}

impl LaidOutSequence {
    /// Estimate the heap size of this sequence for memory monitoring.
    pub fn rough_heap_size(&self) -> usize {
        let mut size = 0;
        size += self.pages.capacity() * std::mem::size_of::<Vec<PositionedElement>>();
        for page in &self.pages {
            size += page.capacity() * std::mem::size_of::<PositionedElement>();
            for el in page {
                if let LayoutElement::Text(t) = &el.element {
                    size += t.content.capacity();
                }
            }
        }
        for (k, v) in &self.resources {
            size += k.capacity();
            size += v.len();
        }
        size
    }
}
