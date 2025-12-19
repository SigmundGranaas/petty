//! Shared data types that bridge layout and render phases.
//!
//! These types represent the output of the layout phase and are consumed
//! by the rendering phase.

use crate::core::idf::SharedData;
use crate::core::layout::{AnchorLocation, IndexEntry, PositionedElement};
use serde::Serialize;
use std::collections::HashMap;

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
                if let crate::core::layout::LayoutElement::Text(t) = &el.element {
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

/// An entry in the table of contents.
#[derive(Debug, Clone, Default)]
pub struct TocEntry {
    /// Heading level (1 = h1, 2 = h2, etc.)
    pub level: u8,
    /// The text content of the heading.
    pub text: String,
    /// The anchor ID to link to this heading.
    pub target_id: String,
}

/// Represents an entry for a document index (for API/serialization).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiIndexEntry {
    /// The text of the index entry.
    pub text: String,
    /// The page number on which this entry is referenced.
    pub page_number: usize,
}
