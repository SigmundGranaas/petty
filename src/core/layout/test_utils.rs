// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/test_utils.rs
#![cfg(test)]

use crate::core::idf::{IRNode, InlineNode, NodeMetadata};
use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::fonts::FontManager;
use crate::core::layout::{LayoutElement, PositionedElement, TextElement};
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use std::sync::Arc;

/// Creates a default layout engine for testing purposes.
pub fn create_test_engine() -> LayoutEngine {
    let mut font_manager = FontManager::new();
    font_manager.load_fallback_font();
    LayoutEngine::new(Arc::new(font_manager))
}

/// A convenience function to run the pagination process for a test.
pub fn paginate_test_nodes(
    stylesheet: Stylesheet,
    nodes: Vec<IRNode>,
) -> Result<Vec<Vec<PositionedElement>>, PipelineError> {
    let engine = create_test_engine();
    engine.paginate(&stylesheet, nodes).map(|(pages, _anchors)| pages)
}

/// Creates a simple paragraph node for testing, converting `\n` to line breaks.
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


/// Finds the first drawable text element on a page that contains the given substring.
pub fn find_first_text_box_with_content<'a>(
    elements: &'a [PositionedElement],
    content: &str,
) -> Option<&'a PositionedElement> {
    elements.iter().find(|el| {
        if let LayoutElement::Text(TextElement {
                                       content: text_content, ..
                                   }) = &el.element
        {
            text_content.contains(content)
        } else {
            false
        }
    })
}