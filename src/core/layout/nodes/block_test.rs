// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/block_test.rs
#![cfg(test)]

use crate::core::idf::{IRNode, LayoutUnit};
use crate::core::layout::test_utils::{
    create_paragraph, create_test_engine_with_page, find_first_text_box_with_content,
};
use crate::core::style::dimension::Margins;
use crate::core::style::stylesheet::ElementStyle;
use serde_json::Value;
use std::sync::Arc;

#[test]
fn test_block_with_padding_indents_child() {
    let engine = create_test_engine_with_page(500.0, 500.0, 10.0);
    let block_style = ElementStyle {
        padding: Some(Margins {
            left: 25.0,
            ..Default::default()
        }),
        ..Default::default()
    };
    let tree = IRNode::Root(vec![IRNode::Block {
        style_sets: vec![],
        style_override: Some(block_style),
        children: vec![create_paragraph("Indented text.")],
    }]);

    let layout_unit = LayoutUnit {
        tree,
        context: Arc::new(Value::Null),
    };
    let pages = engine.paginate_tree(layout_unit).unwrap();
    let page1 = &pages[0];
    let text_el = find_first_text_box_with_content(page1, "Indented").unwrap();

    // Expected X = page margin (10) + block padding (25) = 35
    assert_eq!(text_el.x, 35.0);
}

#[test]
fn test_block_splits_across_pages() {
    // Page content height = 80. Line height is 14.4. 5 lines fit.
    let engine = create_test_engine_with_page(500.0, 100.0, 10.0);
    let tree = IRNode::Root(vec![IRNode::Block {
        style_sets: vec![],
        style_override: None,
        children: vec![
            create_paragraph("Line 1\nLine 2\nLine 3"), // 3 lines
            create_paragraph("Line 4\nLine 5\nLine 6"), // 3 lines
        ],
    }]);

    let layout_unit = LayoutUnit {
        tree,
        context: Arc::new(Value::Null),
    };
    let pages = engine.paginate_tree(layout_unit).unwrap();

    assert_eq!(pages.len(), 2, "Expected two pages");

    // Page 1 should have the first paragraph and the first two lines of the second.
    let page1 = &pages[0];
    assert_eq!(page1.len(), 5);
    assert!(find_first_text_box_with_content(page1, "Line 3").is_some());
    assert!(find_first_text_box_with_content(page1, "Line 5").is_some());
    assert!(find_first_text_box_with_content(page1, "Line 6").is_none());

    // Page 2 should have the last line of the second paragraph.
    let page2 = &pages[1];
    assert_eq!(page2.len(), 1);
    let line6 = find_first_text_box_with_content(page2, "Line 6").unwrap();
    assert_eq!(line6.y, 10.0); // Should be at the top of the new page.
}