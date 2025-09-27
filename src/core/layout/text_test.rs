// FILE: src/core/layout/text_test.rs

use super::style::ComputedStyle;
use super::subtree::measure_subtree_height;
use super::test_utils::{create_layout_unit, create_test_engine};
use crate::core::idf::{IRNode, InlineNode};
use crate::core::style::stylesheet::ElementStyle;
use crate::core::style::text::TextAlign;
use std::sync::Arc;

#[test]
fn test_word_wrapping() {
    let engine = create_test_engine();
    let base_style = Arc::new(ComputedStyle::default()); // font_size: 12.0, line_height: 14.4

    let long_text = "This is a very long sentence that is absolutely guaranteed to wrap onto multiple lines when put into a reasonably narrow container.";
    let mut tree = IRNode::Paragraph {
        style_sets: vec![],
        style_override: None,
        children: vec![InlineNode::Text(long_text.to_string())],
    };

    // With a width of 200, this text should wrap to multiple lines.
    let measured_height = measure_subtree_height(&engine, &mut tree, &base_style, 200.0);

    // A single line is 14.4. We expect at least 3 lines.
    assert!(measured_height > 14.4 * 2.0, "Text should have wrapped to multiple lines, height was {}", measured_height);
}

#[test]
fn test_text_alignment() {
    let engine = create_test_engine();
    let left_margin = engine.stylesheet.page.margins.left;
    let tree = IRNode::Root(vec![
        IRNode::Paragraph { // Left (default)
            style_sets: vec![], style_override: None,
            children: vec![InlineNode::Text("Left".to_string())]
        },
        IRNode::Paragraph { // Center
            style_sets: vec![],
            style_override: Some(ElementStyle { text_align: Some(TextAlign::Center), ..Default::default() }),
            children: vec![InlineNode::Text("Center".to_string())]
        },
        IRNode::Paragraph { // Right
            style_sets: vec![],
            style_override: Some(ElementStyle { text_align: Some(TextAlign::Right), ..Default::default() }),
            children: vec![InlineNode::Text("Right".to_string())]
        },
    ]);

    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
    let page = page_iter.next().unwrap();

    assert_eq!(page.len(), 3);

    // FIX: Left-aligned text should start at the page's left content margin.
    let left_el = &page[0];
    assert!((left_el.x - left_margin).abs() < 0.01, "Left-aligned text should start at the left margin. Got x: {}", left_el.x);

    // Center-aligned text should be significantly indented.
    let center_el = &page[1];
    assert!(center_el.x > 100.0, "Center-aligned text has unexpected x: {}", center_el.x);

    // Right-aligned text should be the most indented.
    let right_el = &page[2];
    assert!(right_el.x > center_el.x, "Right-aligned text has unexpected x: {}", right_el.x);
}

#[test]
fn test_explicit_line_break() {
    let engine = create_test_engine();
    let base_style = Arc::new(ComputedStyle::default());
    let mut tree = IRNode::Paragraph {
        style_sets: vec![],
        style_override: None,
        children: vec![
            InlineNode::Text("First line.".to_string()),
            InlineNode::LineBreak,
            InlineNode::Text("Second line.".to_string()),
        ],
    };

    let measured_height = measure_subtree_height(&engine, &mut tree, &base_style, 500.0);
    let expected_height = base_style.line_height * 2.0;

    assert!((measured_height - expected_height).abs() < 0.01, "Expected height for two lines, but got {}", measured_height);
}