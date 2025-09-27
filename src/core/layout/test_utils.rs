// FILE: src/core/layout/test_utils.rs

#![cfg(test)]

use crate::core::idf::{IRNode, InlineNode, LayoutUnit};
use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::fonts::FontManager;
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
            margins: Margins {
                top: margin,
                right: margin,
                bottom: margin,
                left: margin,
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let mut font_manager = FontManager::new();
    font_manager.load_fallback_font().unwrap();
    LayoutEngine::new(stylesheet, Arc::new(font_manager))
}

/// Creates a simple paragraph node for testing.
pub fn create_paragraph(text: &str) -> IRNode {
    IRNode::Paragraph {
        style_sets: vec![],
        style_override: None,
        children: vec![InlineNode::Text(text.to_string())],
    }
}

/// Wraps an IRNode tree in a LayoutUnit for pagination.
pub fn create_layout_unit(tree: IRNode) -> LayoutUnit {
    LayoutUnit {
        tree,
        context: Arc::new(Value::Null),
    }
}