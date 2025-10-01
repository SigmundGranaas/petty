// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/integration_test.rs
use crate::core::idf::{IRNode, InlineNode, LayoutUnit};
use crate::core::layout::test_utils::{
    create_paragraph, create_test_engine, create_test_engine_with_page,
    find_first_text_box_with_content,
};
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::stylesheet::ElementStyle;
use serde_json::Value;
use std::sync::Arc;

#[test]
fn test_nested_blocks_with_padding_and_margin() {
    let engine = create_test_engine();
    let tree = IRNode::Root(vec![IRNode::Block {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            margin: Some(Margins { top: 5.0, ..Default::default() }),
            padding: Some(Margins { top: 2.0, ..Default::default() }),
            ..Default::default()
        }),
        children: vec![IRNode::Paragraph {
            style_sets: vec![],
            style_override: Some(ElementStyle {
                margin: Some(Margins { top: 10.0, ..Default::default() }),
                ..Default::default()
            }),
            children: vec![InlineNode::Text("Hello".to_string())],
        }],
    }]);

    let layout_unit = LayoutUnit {
        tree,
        context: Arc::new(Value::Null),
    };
    let pages = engine.paginate_tree(layout_unit).unwrap();
    let page1 = &pages[0];

    assert_eq!(page1.len(), 1);
    let text_element = &page1[0];

    // Expected Y position breakdown:
    // Page margin top: 72.0 (from default stylesheet in create_test_engine)
    // Outer block margin top: 5.0
    // Outer block padding top: 2.0
    // Inner paragraph margin top: 10.0
    // Total Y = 72.0 + 5.0 + 2.0 + 10.0 = 89.0
    let expected_y = 89.0;

    assert!(
        (text_element.y - expected_y).abs() < 0.01,
        "Expected y={}, got {}",
        expected_y,
        text_element.y
    );
}

#[test]
fn test_flex_container_with_percentages() {
    // Page width 520, margin 10 -> content width 500
    let engine = create_test_engine_with_page(520.0, 500.0, 10.0);
    let tree = IRNode::Root(vec![IRNode::FlexContainer {
        style_sets: vec![],
        style_override: None,
        children: vec![
            IRNode::Block {
                // Left Column
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Percent(30.0)),
                    ..Default::default()
                }),
                children: vec![create_paragraph("Left")],
            },
            IRNode::Block {
                // Right Column
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Percent(70.0)),
                    padding: Some(Margins { left: 10.0, ..Default::default() }),
                    ..Default::default()
                }),
                children: vec![create_paragraph("Right")],
            },
        ],
    }]);

    let layout_unit = LayoutUnit {
        tree,
        context: Arc::new(Value::Null),
    };
    let pages = engine.paginate_tree(layout_unit).unwrap();
    let page1 = &pages[0];

    let left_text = find_first_text_box_with_content(page1, "Left").unwrap();
    let right_text = find_first_text_box_with_content(page1, "Right").unwrap();

    // Content width is 500.
    // Left text starts at page margin
    assert!((left_text.x - 10.0).abs() < 0.01);

    // Right text should start after the left block (30% of 500 = 150)
    // plus its own block's padding (10.0).
    // X = page_margin(10) + left_block_width(150) + right_block_padding(10)
    let expected_right_x = 10.0 + (500.0 * 0.3) + 10.0;
    assert!(
        (right_text.x - expected_right_x).abs() < 0.01,
        "Right text x was {}, expected {}",
        right_text.x,
        expected_right_x
    );
}