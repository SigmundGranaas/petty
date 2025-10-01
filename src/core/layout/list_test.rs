// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/list_test.rs
// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/list_test.rs
#![cfg(test)]

use super::test_utils::{create_paragraph, paginate_test_nodes};
use crate::core::idf::IRNode;
use crate::core::layout::{LayoutElement, TextElement};
use crate::core::style::dimension::{Margins, PageSize};
use crate::core::style::list::ListStyleType;
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use std::collections::HashMap;

fn create_list_item(text: &str) -> IRNode {
    IRNode::ListItem {
        style_sets: vec![],
        style_override: None,
        children: vec![create_paragraph(text)],
    }
}

fn get_text_content(element: &LayoutElement) -> &str {
    if let LayoutElement::Text(TextElement { content, .. }) = element {
        content
    } else {
        ""
    }
}

#[test]
fn test_unordered_list_layout() {
    let mut stylesheet = Stylesheet::default();
    stylesheet.page_masters.insert("master".to_string(), PageLayout::default());
    stylesheet.default_page_master_name = Some("master".to_string());

    let nodes = vec![IRNode::List {
        style_sets: vec![],
        style_override: None,
        children: vec![create_list_item("Item 1"), create_list_item("Item 2")],
    }];

    let mut pages = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page = pages.remove(0);

    // Each item produces a marker box and a text box.
    assert_eq!(page.len(), 4);

    let marker1 = &page[0];
    let text1 = &page[1];
    let marker2 = &page[2];
    let text2 = &page[3];

    assert_eq!(get_text_content(&marker1.element), "•");
    assert_eq!(get_text_content(&text1.element), "Item 1");
    assert_eq!(get_text_content(&marker2.element), "•");
    assert_eq!(get_text_content(&text2.element), "Item 2");

    // Check indentation: text should be to the right of the marker.
    assert!(text1.x > marker1.x);
    // Markers and text blocks for each item should align vertically.
    assert_eq!(marker1.x, marker2.x);
    assert_eq!(text1.x, text2.x);

    // Check vertical stacking: item 2 should be below item 1.
    assert!(marker2.y > marker1.y);
    assert_eq!(marker1.y, text1.y); // Marker and text are on the same line
    assert_eq!(marker2.y, text2.y);
}

#[test]
fn test_ordered_list_layout() {
    let mut stylesheet = Stylesheet::default();
    stylesheet.page_masters.insert("master".to_string(), PageLayout::default());
    stylesheet.default_page_master_name = Some("master".to_string());

    let nodes = vec![IRNode::List {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            list_style_type: Some(ListStyleType::Decimal),
            ..Default::default()
        }),
        children: vec![create_list_item("First"), create_list_item("Second")],
    }];

    let mut pages = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page = pages.remove(0);

    assert_eq!(page.len(), 4);
    assert_eq!(get_text_content(&page[0].element), "1.");
    assert_eq!(get_text_content(&page[1].element), "First");
    assert_eq!(get_text_content(&page[2].element), "2.");
    assert_eq!(get_text_content(&page[3].element), "Second");

    assert!(page[1].x > page[0].x);
    assert!(page[3].x > page[2].x);
    assert!(page[2].y > page[0].y);
}

#[test]
fn test_list_pagination() {
    // Page content height is 70 (90H - 10T - 10B). Line height is 14.4.
    // 70 / 14.4 = 4.86. Should fit 4 items.
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 90.0 },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let mut children = vec![];
    for i in 1..=10 {
        children.push(create_list_item(&format!("List item {}", i)));
    }
    let nodes = vec![IRNode::List {
        style_sets: vec![],
        style_override: None,
        children,
    }];
    let mut pages = paginate_test_nodes(stylesheet, nodes).unwrap();

    // Page 1
    let page1 = pages.remove(0);
    assert_eq!(page1.len(), 4 * 2, "Page 1 should have 4 items (8 elements)");
    assert_eq!(get_text_content(&page1[7].element), "List item 4");

    // Page 2
    let page2 = pages.remove(0);
    assert_eq!(page2.len(), 4 * 2, "Page 2 should have next 4 items");
    let item5_text = &page2[1];
    assert_eq!(get_text_content(&item5_text.element), "List item 5");
    assert_eq!(item5_text.y, 10.0);

    // Page 3
    let page3 = pages.remove(0);
    assert_eq!(page3.len(), 2 * 2, "Page 3 should have remaining 2 items");
    let item9_text = &page3[1];
    assert_eq!(get_text_content(&item9_text.element), "List item 9");
    assert_eq!(item9_text.y, 10.0);

    assert!(pages.is_empty());
}