// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/integration_test.rs
use crate::core::idf::{IRNode, InlineNode, NodeMetadata};
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use std::collections::HashMap;

#[test]
fn test_nested_blocks_with_padding_and_margin() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::A4,
                margins: Some(Margins::all(72.0)), // 1 inch
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let nodes = vec![IRNode::Block {
        meta: NodeMetadata {
            style_override: Some(ElementStyle {
                margin: Some(Margins { top: 5.0, ..Default::default() }),
                padding: Some(Margins { top: 2.0, ..Default::default() }),
                ..Default::default()
            }),
            ..Default::default()
        },
        children: vec![IRNode::Paragraph {
            meta: NodeMetadata {
                style_override: Some(ElementStyle {
                    margin: Some(Margins { top: 10.0, ..Default::default() }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            children: vec![InlineNode::Text("Hello".to_string())],
        }],
    }];

    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    assert_eq!(page1.len(), 1);
    let text_element = &page1[0];

    // Expected Y position breakdown:
    // Page margin top: 72.0
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
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 520.0, height: 500.0 },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let nodes = vec![IRNode::FlexContainer {
        meta: Default::default(),
        children: vec![
            IRNode::Block {
                // Left Column
                meta: NodeMetadata {
                    style_override: Some(ElementStyle {
                        width: Some(Dimension::Percent(30.0)),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                children: vec![create_paragraph("Left")],
            },
            IRNode::Block {
                // Right Column
                meta: NodeMetadata {
                    style_override: Some(ElementStyle {
                        width: Some(Dimension::Percent(70.0)),
                        padding: Some(Margins { left: 10.0, ..Default::default() }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                children: vec![create_paragraph("Right")],
            },
        ],
    }];

    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();
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