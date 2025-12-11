use crate::core::idf::{IRNode, InlineNode, NodeMetadata};
use crate::core::layout::engine::{LayoutEngine, LayoutStore};
use crate::core::layout::fonts::SharedFontLibrary;
use crate::core::layout::node::{AnchorLocation, IndexEntry};
use crate::core::layout::{LayoutElement, PositionedElement, TextElement};
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use std::collections::HashMap;

/// Creates a default layout engine for testing purposes.
pub fn create_test_engine() -> LayoutEngine {
    let library = SharedFontLibrary::new();
    library.load_fallback_font();
    LayoutEngine::new(&library, Default::default())
}

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
    let store = LayoutStore::new();
    let ir_root = IRNode::Root(nodes);

    let root = engine.build_render_tree(&ir_root, &store)
        .map_err(|e| PipelineError::Layout(e.to_string()))?;

    let iterator = engine
        .paginate(&stylesheet, root, &store)
        .map_err(|e| PipelineError::Layout(e.to_string()))?;

    let mut pages = Vec::new();
    let mut all_anchors = HashMap::new();
    let mut all_index_entries = HashMap::new();

    for page_result in iterator {
        let page = page_result.map_err(|e| PipelineError::Layout(e.to_string()))?;
        pages.push(page.elements);
        all_anchors.extend(page.anchors);

        for (key, mut entries) in page.index_entries {
            all_index_entries
                .entry(key)
                .or_insert_with(Vec::new)
                .append(&mut entries);
        }
    }

    Ok((pages, all_anchors, all_index_entries))
}

pub fn create_paragraph(text: &str) -> IRNode {
    let mut children = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            children.push(InlineNode::LineBreak);
        }
        if !line.is_empty() {
            children.push(InlineNode::Text(line.to_string()));
        }
    }

    IRNode::Paragraph {
        meta: NodeMetadata::default(),
        children,
    }
}

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