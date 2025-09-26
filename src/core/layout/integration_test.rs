//! Integration tests for the layout engine's non-paginated subtree layout logic.
//! These tests verify that different IRNode types compose and calculate their
//! dimensions correctly.

use super::style::ComputedStyle;
use super::subtree::{layout_subtree, measure_subtree_height};
use super::{IRNode, LayoutEngine, FontManager};
use std::sync::Arc;
use crate::core::idf::{InlineNode, TableBody, TableCell, TableColumnDefinition, TableRow};
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};

fn create_test_engine() -> LayoutEngine {
    let stylesheet = Stylesheet::default();
    let mut font_manager = FontManager::new();
    font_manager.load_fallback_font().unwrap();
    LayoutEngine::new(stylesheet, Arc::new(font_manager))
}

fn get_base_style() -> Arc<ComputedStyle> {
    let engine = create_test_engine();
    let mut style = (*engine.get_default_style()).clone();
    style.font_size = 10.0;
    style.line_height = 12.0;
    Arc::new(style)
}

#[test]
fn test_subtree_nested_blocks() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let mut tree = IRNode::Block {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            margin: Some(Margins { top: 5.0, ..Default::default() }),
            padding: Some(Margins { top: 2.0, bottom: 2.0, ..Default::default() }),
            ..Default::default()
        }),
        children: vec![IRNode::Paragraph {
            style_sets: vec![],
            style_override: Some(ElementStyle {
                margin: Some(Margins { top: 10.0, bottom: 10.0, ..Default::default() }),
                ..Default::default()
            }),
            children: vec![InlineNode::Text("Hello".to_string())],
        }],
    };

    // Correct Expected Height Breakdown:
    // Inner Paragraph total height = margin_top (10) + content (12) + margin_bottom (10) = 32.0
    // Outer Block content height = padding_top (2) + child_height (32) + padding_bottom (2) = 36.0
    // Outer Block total height = margin_top (5) + content_height (36) + margin_bottom (0) = 41.0
    let expected_height = 41.0;

    let measured_height = measure_subtree_height(&engine, &mut tree, &base_style, 500.0);
    assert!((measured_height - expected_height).abs() < 0.01);

    let (elements, total_height) = layout_subtree(&engine, &mut tree, &base_style, 500.0);
    assert!((total_height - expected_height).abs() < 0.01);
    assert_eq!(elements.len(), 1);
    let text_el = &elements[0];

    // Y position relative to the start of the outer block's total area
    // Y = outer_margin_top (5) + outer_padding_top (2) + inner_margin_top (10) = 17.0
    assert_eq!(text_el.y, 17.0);
}

#[test]
fn test_subtree_list_with_items() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let mut tree = IRNode::List {
        style_sets: vec![],
        style_override: None,
        children: vec![
            IRNode::ListItem {
                style_sets: vec![],
                style_override: None,
                children: vec![IRNode::Paragraph {
                    style_sets: vec![],
                    style_override: None,
                    children: vec![InlineNode::Text("Item 1".to_string())],
                }],
            },
            IRNode::ListItem {
                style_sets: vec![],
                style_override: None,
                children: vec![IRNode::Paragraph {
                    style_sets: vec![],
                    style_override: None,
                    children: vec![InlineNode::Text("Item 2".to_string())],
                }],
            },
        ],
    };

    // Each ListItem contains one paragraph (height 12.0)
    // Total height = 12.0 + 12.0 = 24.0
    let measured_height = measure_subtree_height(&engine, &mut tree, &base_style, 500.0);
    assert!((measured_height - 24.0).abs() < 0.01);

    let (elements, total_height) = layout_subtree(&engine, &mut tree, &base_style, 500.0);
    assert!((total_height - 24.0).abs() < 0.01);
    assert_eq!(elements.len(), 4);

    let first_text = &elements[1];
    let bullet_width = base_style.font_size * 0.6; // Use the same hardcoded ratio as the implementation
    let bullet_spacing = base_style.font_size * 0.4;
    assert!((first_text.x - (bullet_width + bullet_spacing)).abs() < 0.01);
    assert_eq!(first_text.y, 0.0);

    let second_text = &elements[3];
    assert_eq!(second_text.y, 12.0);
}

#[test]
fn test_subtree_flex_container() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let mut tree = IRNode::FlexContainer {
        style_sets: vec![],
        style_override: None,
        children: vec![
            IRNode::Block { // Tall block
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(100.0)),
                    ..Default::default()
                }),
                children: vec![
                    IRNode::Paragraph {style_sets: vec![], style_override: None, children: vec![InlineNode::Text("1".into())]}, // Height 12.0
                    IRNode::Paragraph {style_sets: vec![], style_override: None, children: vec![InlineNode::Text("2".into())]}, // Height 12.0
                ],
            },
            IRNode::Block { // Short block
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(100.0)),
                    ..Default::default()
                }),
                children: vec![
                    IRNode::Paragraph {style_sets: vec![], style_override: None, children: vec![InlineNode::Text("3".into())]}, // Height 12.0
                ],
            },
        ],
    };

    // Tallest child has 2 paragraphs, so its total height is 12.0 + 12.0 = 24.0
    // Flex container height is max of children heights.
    let expected_height = 24.0;

    // Run measurement pass first on a clone, to ensure layout pass gets a clean tree.
    let mut tree_for_measure = tree.clone();
    let measured_height = measure_subtree_height(&engine, &mut tree_for_measure, &base_style, 500.0);
    assert!((measured_height - expected_height).abs() < 0.01);

    // Now run layout pass on the original tree
    let (elements, total_height) = layout_subtree(&engine, &mut tree, &base_style, 500.0);
    assert!((total_height - expected_height).abs() < 0.01);
    assert_eq!(elements.len(), 3);

    let third_para_el = &elements[2];
    assert_eq!(third_para_el.x, 100.0);
    assert_eq!(third_para_el.y, 0.0);
}

#[test]
fn test_subtree_table_layout() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let mut tree = IRNode::Table {
        style_sets: vec![],
        style_override: None,
        columns: vec![TableColumnDefinition {
            width: Some(Dimension::Pt(100.0)),
            ..Default::default()
        }],
        calculated_widths: vec![100.0],
        header: None,
        body: Box::new(TableBody {
            rows: vec![TableRow {
                cells: vec![TableCell {
                    style_sets: vec![],
                    style_override: None,
                    children: vec![IRNode::Paragraph {
                        style_sets: vec![],
                        style_override: None,
                        children: vec![InlineNode::Text("Cell Content".to_string())],
                    }],
                }],
            }],
        }),
    };

    let measured_height = measure_subtree_height(&engine, &mut tree, &base_style, 500.0);
    assert!((measured_height - 12.0).abs() < 0.01);

    let (elements, total_height) = layout_subtree(&engine, &mut tree, &base_style, 500.0);
    assert!((total_height - 12.0).abs() < 0.01);
    assert_eq!(elements.len(), 1);
}

#[test]
fn test_subtree_flex_container_with_percentages() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let container_width = 500.0;

    let mut tree = IRNode::FlexContainer {
        style_sets: vec![],
        style_override: None,
        children: vec![
            IRNode::Block { // Left Column
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Percent(30.0)), // 30% of 500 = 150
                    padding: Some(Margins { right: 10.0, ..Default::default() }), // content width = 140
                    ..Default::default()
                }),
                children: vec![
                    IRNode::Paragraph {
                        style_sets: vec![], style_override: None,
                        children: vec![InlineNode::Text("Left column.".into())]
                    }
                ],
            },
            IRNode::Block { // Right Column
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Percent(70.0)), // 70% of 500 = 350
                    padding: Some(Margins { left: 10.0, ..Default::default() }), // content width = 340
                    ..Default::default()
                }),
                children: vec![
                    IRNode::Paragraph {
                        style_sets: vec![], style_override: None,
                        children: vec![InlineNode::Text("Wider right column.".into())]
                    }
                ],
            },
        ],
    };

    let (elements, total_height) = layout_subtree(&engine, &mut tree, &base_style, container_width);

    assert_eq!(elements.len(), 2);
    assert!(total_height > 0.0);

    let left_text = &elements[0];
    let right_text = &elements[1];

    // Left text should be at x=0 (relative to its block's content area)
    assert_eq!(left_text.x, 0.0);
    // Right text should start after the first column's width (150) plus its own padding (10)
    assert!((right_text.x - 160.0).abs() < 0.01, "Expected right_text.x to be ~160.0, but it was {}", right_text.x);
}


#[test]
fn test_subtree_nested_flex_and_block_padding() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let container_width = 500.0;

    // A flex container with two children. The right child is a block with its own padding.
    let mut tree = IRNode::FlexContainer {
        style_sets: vec![],
        style_override: None,
        children: vec![
            IRNode::Block { // Left Column
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(200.0)),
                    ..Default::default()
                }),
                children: vec![IRNode::Paragraph {
                    style_sets: vec![], style_override: None,
                    children: vec![InlineNode::Text("Left".into())]
                }],
            },
            IRNode::Block { // Right Column
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(300.0)),
                    padding: Some(Margins { left: 20.0, right: 20.0, ..Default::default() }), // 40pt padding
                    ..Default::default()
                }),
                children: vec![
                    IRNode::Paragraph { // This paragraph's content width should be 300 - 40 = 260
                        style_sets: vec![], style_override: None,
                        children: vec![InlineNode::Text("This is some longer text that should definitely wrap within the padded right column content area.".into())]
                    }
                ],
            },
        ],
    };

    let (elements, total_height) = layout_subtree(&engine, &mut tree, &base_style, container_width);

    assert!(!elements.is_empty());
    // The text in the right column has to wrap, so the height should be for at least two lines.
    assert!(total_height > 12.0, "Height should be > 1 line due to wrapping. Got {}", total_height);

    let left_text = &elements[0];
    let right_text_elements: Vec<_> = elements.iter().filter(|el| el.x >= 200.0).collect();

    // Left text starts at x=0 (relative to container) + its own block's padding (0) = 0
    assert_eq!(left_text.x, 0.0);

    // Right text starts at x=200 (flex offset) + its own block's padding (20) = 220
    assert!((right_text_elements[0].x - 220.0).abs() < 0.01, "Expected right text x to be ~220.0, got {}", right_text_elements[0].x);

    // The text element's width should be constrained by the padding.
    // The available width for the paragraph was 260. The text element width should be <= 260.
    assert!(right_text_elements[0].width <= 260.0, "Text width {} should be <= 260", right_text_elements[0].width);
}