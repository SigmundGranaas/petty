// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/test_utils.rs
#![cfg(test)]

use crate::core::idf::{IRNode, InlineNode, LayoutUnit};
use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::fonts::FontManager;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutElement, PositionedElement, TextElement};
use crate::core::style::dimension::{Margins, PageSize};
use crate::core::style::stylesheet::{PageLayout, Stylesheet};
use serde_json::Value;
use std::sync::Arc;

/// Creates a default layout engine for testing purposes.
pub fn create_test_engine() -> LayoutEngine {
    let stylesheet = Stylesheet::default();
    let mut font_manager = FontManager::new();
    font_manager.load_fallback_font().unwrap();
    LayoutEngine::new(stylesheet, Arc::new(font_manager))
}

/// Creates a test engine with specific page dimensions and margins.
pub fn create_test_engine_with_page(width: f32, height: f32, margin: f32) -> LayoutEngine {
    let stylesheet = Stylesheet {
        page: PageLayout {
            size: PageSize::Custom { width, height },
            margins: Some(Margins {
                top: margin,
                right: margin,
                bottom: margin,
                left: margin,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut font_manager = FontManager::new();
    font_manager.load_fallback_font().unwrap();
    LayoutEngine::new(stylesheet, Arc::new(font_manager))
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
        style_sets: vec![],
        style_override: None,
        children,
    }
}

/// Wraps an IRNode tree in a LayoutUnit for pagination.
pub fn create_layout_unit(tree: IRNode) -> LayoutUnit {
    LayoutUnit {
        tree,
        context: Arc::new(Value::Null),
    }
}

/// Creates a base computed style for use in tests.
pub fn get_base_style() -> Arc<ComputedStyle> {
    let engine = create_test_engine();
    let mut style = (*engine.get_default_style()).clone();
    style.font_size = 10.0;
    style.line_height = 12.0;
    Arc::new(style)
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