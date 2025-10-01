// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/flex_test.rs
#![cfg(test)]
use crate::core::idf::IRNode;
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::flex::{FlexWrap, JustifyContent};
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use std::collections::HashMap;

fn create_flex_item(width: f32, height: f32, text: &str) -> IRNode {
    IRNode::Block {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            width: Some(Dimension::Pt(width)),
            height: Some(Dimension::Pt(height)),
            ..Default::default()
        }),
        children: vec![create_paragraph(text)],
    }
}

#[test]
fn test_flex_justify_content() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: 500.0,
                },
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let nodes = vec![IRNode::FlexContainer {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            justify_content: Some(JustifyContent::Center),
            ..Default::default()
        }),
        children: vec![
            create_flex_item(100.0, 20.0, "1"),
            create_flex_item(100.0, 20.0, "2"),
        ],
    }];
    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    let item1 = find_first_text_box_with_content(page1, "1").unwrap();

    // Total width = 200. Container width = 500. Free space = 300.
    // Centered means starting offset is 300 / 2 = 150.
    assert_eq!(item1.x, 150.0);
}

#[test]
fn test_flex_wrap_with_page_break() {
    // Page content height = 50. Each item is 30 high.
    // Line 1: 3 items, height 30. Fits. Cursor at 30.
    // Line 2: 3 items, height 30. Does not fit (30 > 50-30). Page break.
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 350.0,
                    height: 70.0,
                },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let nodes = vec![IRNode::FlexContainer {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            flex_wrap: Some(FlexWrap::Wrap),
            ..Default::default()
        }),
        children: vec![
            create_flex_item(100.0, 30.0, "1"),
            create_flex_item(100.0, 30.0, "2"),
            create_flex_item(100.0, 30.0, "3"), // End of first line
            create_flex_item(100.0, 30.0, "4"), // Start of second line
            create_flex_item(100.0, 30.0, "5"),
        ],
    }];

    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();

    assert_eq!(pages.len(), 2, "Expected flex container to wrap onto a second page");

    // Page 1 should contain items 1, 2, 3
    let page1 = &pages[0];
    assert!(find_first_text_box_with_content(page1, "1").is_some());
    assert!(find_first_text_box_with_content(page1, "3").is_some());
    assert!(find_first_text_box_with_content(page1, "4").is_none());
    let item1_p1 = find_first_text_box_with_content(page1, "1").unwrap();
    assert_eq!(item1_p1.y, 10.0); // Top of the page

    // Page 2 should contain items 4, 5
    let page2 = &pages[1];
    assert!(find_first_text_box_with_content(page2, "3").is_none());
    assert!(find_first_text_box_with_content(page2, "4").is_some());
    assert!(find_first_text_box_with_content(page2, "5").is_some());
    let item4_p2 = find_first_text_box_with_content(page2, "4").unwrap();
    assert_eq!(item4_p2.y, 10.0); // Top of the new page
}