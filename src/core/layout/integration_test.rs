use super::style::ComputedStyle;
use super::{IRNode, LayoutBox, LayoutContent, LayoutEngine};
use crate::core::idf::InlineNode;
use crate::core::layout::test_utils::{create_test_engine, get_base_style};
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::stylesheet::ElementStyle;
use std::sync::Arc;

fn find_first_text_box<'a>(layout_box: &'a LayoutBox) -> Option<&'a LayoutBox> {
    match &layout_box.content {
        LayoutContent::Text(..) => Some(layout_box),
        LayoutContent::Children(children) => {
            children.iter().find_map(find_first_text_box)
        }
        _ => None,
    }
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

    let layout_box = engine.build_layout_tree(&mut tree, base_style, (500.0, f32::INFINITY));

    // Expected Height Breakdown:
    // Inner Paragraph has content height = 12.0 (one line)
    // Inner Paragraph box height (content + padding + margin) = 12.0 + 0 + 0 + 10.0 + 10.0 = 32.0
    // Outer Block content height = paragraph's total height = 32.0
    // Outer Block box height = content_height(32.0) + padding(2+2) + margin(5+0) = 41.0
    let expected_height = 41.0;

    assert!((layout_box.rect.height - expected_height).abs() < 0.01);

    // The layout_box itself has a `y` equal to its top margin.
    assert!((layout_box.rect.y - 5.0).abs() < 0.01);

    // Let's find the paragraph box inside.
    let outer_block_children = match &layout_box.content {
        LayoutContent::Children(c) => c,
        _ => panic!("Outer block should have children"),
    };
    let paragraph_box = &outer_block_children[0];

    // Its Y should be relative to the outer block's content area (i.e., inside padding).
    // Y = outer_padding_top(2.0) + inner_paragraph_margin_top(10.0)
    let expected_y = 2.0 + 10.0;
    assert!((paragraph_box.rect.y - expected_y).abs() < 0.01);
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
                    width: Some(Dimension::Percent(30.0)),
                    padding: Some(Margins { right: 10.0, ..Default::default() }),
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
                    width: Some(Dimension::Percent(70.0)),
                    padding: Some(Margins { left: 10.0, ..Default::default() }),
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

    let layout_box = engine.build_layout_tree(&mut tree, base_style.clone(), (container_width, f32::INFINITY));

    assert!(layout_box.rect.height > 0.0);
    if let LayoutContent::Children(children) = layout_box.content {
        assert_eq!(children.len(), 2, "Expected 2 flex items");
        let left_item = &children[0];
        let right_item = &children[1];

        // Left item should have a width of 150 (30% of 500)
        assert!((left_item.rect.width - 150.0).abs() < 0.01, "Left item width was {}", left_item.rect.width);
        assert_eq!(left_item.rect.x, 0.0);

        // Right item should have a width of 350 (70% of 500) and start after the left one.
        assert!((right_item.rect.width - 350.0).abs() < 0.01);
        assert!((right_item.rect.x - 150.0).abs() < 0.01);

        // --- CORRECTED TEST LOGIC ---
        // Find the paragraph box inside the right item to check padding.
        let paragraph_box = match &right_item.content {
            LayoutContent::Children(item_children) => item_children.get(0).expect("Right item should have a paragraph"),
            _ => panic!("Right item should have children"),
        };

        // The paragraph's `x` should be indented by the parent block's padding.
        assert!((paragraph_box.rect.x - 10.0).abs() < 0.01, "Paragraph inside right item was not indented by padding. Got {}", paragraph_box.rect.x);

        // Find the text box inside the paragraph.
        let text_box = find_first_text_box(paragraph_box).unwrap();

        // The text box's `x` should be 0, relative to the paragraph it's in.
        assert!((text_box.rect.x - 0.0).abs() < 0.01, "Text inside paragraph was not at its container's origin. Got {}", text_box.rect.x);

    } else {
        panic!("Flex container should have children");
    }
}