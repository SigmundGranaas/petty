#![cfg(test)]

use crate::core::idf::{IRNode, InlineNode, NodeMetadata};
use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::fonts::SharedFontLibrary;
use crate::core::layout::node::{AnchorLocation, IndexEntry};
use crate::core::layout::{LayoutElement, PositionedElement, TextElement};
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use std::collections::HashMap;
use bumpalo::Bump;

/// Creates a default layout engine for testing purposes.
pub fn create_test_engine() -> LayoutEngine {
    let mut library = SharedFontLibrary::new();
    library.load_fallback_font();
    // In tests, LayoutEngine is created with a reference to the library.
    // Since we need to return ownership of the engine, we can make the library
    // outlive the engine in this specific scope, OR we construct the engine
    // by leaking the library or using a static one.
    // For test convenience, we create a fresh one. Note that this function
    // is slightly problematic if LayoutEngine stores &SharedFontLibrary.
    // However, LayoutEngine now OWNS LocalFontContext which copies data from SharedFontLibrary.
    LayoutEngine::new(&library)
}

/// A convenience function to run the pagination process for a test.
///
/// This helper abstracts the iterator pattern used by `LayoutEngine::paginate`,
/// collecting all pages into a Vector for easy assertions in tests.
pub fn paginate_test_nodes(
    stylesheet: Stylesheet,
    nodes: Vec<IRNode>,
) -> Result<
    (
        Vec<Vec<PositionedElement>>,
        HashMap<String, AnchorLocation>,
        HashMap<String, Vec<IndexEntry>>,
    ),
    PipelineError,
> {
    let engine = create_test_engine();
    let arena = Bump::new();
    let ir_root = IRNode::Root(nodes);

    let root = engine.build_render_tree(&ir_root, &arena)
        .map_err(|e| PipelineError::Layout(e.to_string()))?;

    let iterator = engine
        .paginate(&stylesheet, root, &arena)
        .map_err(|e| PipelineError::Layout(e.to_string()))?;

    let mut pages = Vec::new();
    let mut all_anchors = HashMap::new();
    let mut all_index_entries = HashMap::new();

    for page_result in iterator {
        let page = page_result.map_err(|e| PipelineError::Layout(e.to_string()))?;
        pages.push(page.elements);
        all_anchors.extend(page.anchors);

        // Merging index entries is a bit more complex since it's a Vec
        for (key, mut entries) in page.index_entries {
            all_index_entries
                .entry(key)
                .or_insert_with(Vec::new)
                .append(&mut entries);
        }
    }

    Ok((pages, all_anchors, all_index_entries))
}

/// Creates a simple paragraph node for testing, converting `\n` to line breaks.
pub fn create_paragraph(text: &str) -> IRNode {
    let mut children = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            children.push(InlineNode::LineBreak);
        }
        // Don't skip empty lines entirely if they are explicitly part of split,
        // but for simple text paragraphs, empty strings usually mean consecutive delimiters.
        if !line.is_empty() {
            children.push(InlineNode::Text(line.to_string()));
        }
    }

    IRNode::Paragraph {
        meta: NodeMetadata::default(),
        children,
    }
}

/// Finds the first drawable text element on a page that contains the given substring.
pub fn find_first_text_box_with_content<'a>(
    elements: &'a [PositionedElement],
    content: &str,
) -> Option<&'a PositionedElement> {
    elements.iter().find(|el| {
        if let LayoutElement::Text(TextElement {
                                       content: text_content,
                                       ..
                                   }) = &el.element
        {
            text_content.contains(content)
        } else {
            false
        }
    })
}