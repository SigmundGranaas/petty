#![cfg(test)]

// Correct import path for test_utils which is in crate::core::layout
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::idf::{IRNode, NodeMetadata};
use crate::core::layout::{LayoutElement, TextElement};
use crate::core::style::dimension::{Margins, PageSize};
use crate::core::style::list::{ListStylePosition, ListStyleType};
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use std::collections::HashMap;

fn create_list_item(text: &str) -> IRNode {
    IRNode::ListItem {
        meta: Default::default(),
        children: vec![create_paragraph(text)],
    }
}

fn create_list_item_with_children(children: Vec<IRNode>) -> IRNode {
    IRNode::ListItem {
        meta: Default::default(),
        children,
    }
}

fn create_list(children: Vec<IRNode>, style: Option<ElementStyle>, start: Option<usize>) -> IRNode {
    IRNode::List {
        meta: NodeMetadata {
            style_override: style,
            ..Default::default()
        },
        start,
        children,
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

    let nodes = vec![create_list(vec![create_list_item("Item 1"), create_list_item("Item 2")], None, None)];

    let (mut pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
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

    let style = ElementStyle {
        list_style_type: Some(ListStyleType::Decimal),
        ..Default::default()
    };
    let nodes = vec![create_list(vec![create_list_item("First"), create_list_item("Second")], Some(style), None)];

    let (mut pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
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
fn test_nested_ordered_list_numbering_cycles_correctly() {
    let mut stylesheet = Stylesheet::default();
    stylesheet.page_masters.insert("master".to_string(), PageLayout::default());
    stylesheet.default_page_master_name = Some("master".to_string());

    let nested_list = create_list(
        vec![
            create_list_item("Item 2a"),
            create_list_item_with_children(vec![
                create_paragraph("Item 2b"),
                create_list(vec![create_list_item("Item 2bi")], None, None),
            ]),
        ],
        None,
        None,
    );

    let top_list_style = ElementStyle { list_style_type: Some(ListStyleType::Decimal), ..Default::default() };
    let nodes = vec![create_list(
        vec![
            create_list_item("Item 1"),
            create_list_item_with_children(vec![create_paragraph("Item 2"), nested_list]),
            create_list_item("Item 3"),
        ],
        Some(top_list_style),
        None,
    )];


    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    let marker1 = find_first_text_box_with_content(page1, "1.").expect("Marker '1.' not found");
    let marker2 = find_first_text_box_with_content(page1, "2.").expect("Marker '2.' not found");
    let marker3 = find_first_text_box_with_content(page1, "3.").expect("Marker '3.' not found");
    let marker2a = find_first_text_box_with_content(page1, "a.").expect("Marker 'a.' not found");
    let marker2b = find_first_text_box_with_content(page1, "b.").expect("Marker 'b.' not found");
    let marker2bi = find_first_text_box_with_content(page1, "i.").expect("Marker 'i.' not found");

    // Check horizontal alignment and indentation
    assert_eq!(marker1.x, marker2.x);
    assert_eq!(marker1.x, marker3.x);
    assert!(marker2a.x > marker1.x, "Nested list 'a.' should be indented");
    assert_eq!(marker2a.x, marker2b.x);
    assert!(marker2bi.x > marker2a.x, "Deeply nested list 'i.' should be further indented");

    // Check vertical stacking order
    assert!(marker2.y > marker1.y);
    assert!(marker2a.y > marker2.y);
    assert!(marker2b.y > marker2a.y);
    assert!(marker2bi.y > marker2b.y);
    assert!(marker3.y > marker2bi.y);
}

#[test]
fn test_list_style_position_inside() {
    let mut stylesheet = Stylesheet::default();
    stylesheet.page_masters.insert("master".to_string(), PageLayout::default());
    stylesheet.default_page_master_name = Some("master".to_string());

    let style = ElementStyle {
        list_style_position: Some(ListStylePosition::Inside),
        ..Default::default()
    };
    let nodes = vec![create_list(vec![create_list_item("Item 1")], Some(style), None)];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    // With 'inside', the marker and text are part of the same text element.
    // There is no separate marker box.
    assert_eq!(page1.len(), 1);
    let text_element = &page1[0];
    let content = get_text_content(&text_element.element);

    assert!(content.starts_with("• Item 1"));
    assert_eq!(text_element.x, 0.0); // Starts at the beginning of the block
}

#[test]
fn test_ordered_list_upper_alpha_roman() {
    let mut stylesheet = Stylesheet::default();
    stylesheet.page_masters.insert("master".to_string(), PageLayout::default());
    stylesheet.default_page_master_name = Some("master".to_string());

    let style_ua = Some(ElementStyle { list_style_type: Some(ListStyleType::UpperAlpha), ..Default::default() });
    let style_ur = Some(ElementStyle { list_style_type: Some(ListStyleType::UpperRoman), ..Default::default() });

    let nodes = vec![
        create_list(vec![create_list_item("A"), create_list_item("B")], style_ua, None),
        create_list(vec![create_list_item("I"), create_list_item("II")], style_ur, None),
    ];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    assert!(find_first_text_box_with_content(page1, "A.").is_some());
    assert!(find_first_text_box_with_content(page1, "B.").is_some());
    assert!(find_first_text_box_with_content(page1, "I.").is_some());
    assert!(find_first_text_box_with_content(page1, "II.").is_some());
}

#[test]
fn test_ordered_list_start_attribute() {
    let mut stylesheet = Stylesheet::default();
    stylesheet.page_masters.insert("master".to_string(), PageLayout::default());
    stylesheet.default_page_master_name = Some("master".to_string());

    let style = Some(ElementStyle { list_style_type: Some(ListStyleType::Decimal), ..Default::default() });
    let nodes = vec![create_list(vec![create_list_item("Third"), create_list_item("Fourth")], style, Some(3))];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    assert!(find_first_text_box_with_content(page1, "3.").is_some());
    assert!(find_first_text_box_with_content(page1, "4.").is_some());
}

#[test]
fn test_list_with_complex_item_splits_correctly() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 80.0 }, // content height ~60
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    // Line height is 14.4. 60 / 14.4 = 4.16. 4 lines fit.
    // The default `widows: 2` would cause a premature break. We set it to 1 to test
    // the pure pagination logic.
    let style_override = Some(ElementStyle { widows: Some(1), ..Default::default() });
    let mut p1 = create_paragraph("Line 1\nLine 2");
    let mut p2 = create_paragraph("Line 3\nLine 4\nLine 5");
    if let IRNode::Paragraph { meta, .. } = &mut p1 { meta.style_override = style_override.clone(); }
    if let IRNode::Paragraph { meta, .. } = &mut p2 { meta.style_override = style_override; }

    let complex_item = create_list_item_with_children(vec![p1, p2]);
    let nodes = vec![create_list(vec![complex_item], None, None)];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    assert_eq!(pages.len(), 2, "Expected list item to split across pages");

    let page1 = &pages[0];
    assert!(find_first_text_box_with_content(page1, "•").is_some(), "Marker should be on page 1");
    assert!(find_first_text_box_with_content(page1, "Line 4").is_some(), "Line 4 should be on page 1");
    assert!(find_first_text_box_with_content(page1, "Line 5").is_none(), "Line 5 should not be on page 1");

    let page2 = &pages[1];
    assert!(find_first_text_box_with_content(page2, "•").is_none(), "Marker should NOT be repeated on page 2");
    let line5 = find_first_text_box_with_content(page2, "Line 5").unwrap();
    assert_eq!(line5.y, 10.0, "Line 5 should be at the top of page 2");
}

#[test]
fn test_list_item_splitting_behavior_bug() {
    // Reproduction of "single word on each page" or aggressive splitting.
    // We create a list item with a paragraph that has multiple lines.
    // We set a page height that fits only ~1.5 lines of text per page after margins/padding.
    // We expect it to split, but not degenerate into infinite 1-word pages unless space is truly that tight.

    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 300.0, height: 40.0 }, // Very short page!
                margins: Some(Margins::all(10.0)), // 20.0 vertical margin -> 20.0 content height
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    // Style with line-height ~14.4.
    // Content height 20.0.
    // 1 line (14.4) fits. 2 lines (28.8) do not.
    // So we expect 1 line per page.
    // This isn't necessarily a bug IF the page is that small.
    // But if the user sees this on a normal page, it means `available_height` is being reported incorrectly.

    let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let p1 = create_paragraph(text);
    let nodes = vec![create_list(vec![create_list_item_with_children(vec![p1])], None, None)];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();

    // With 20pt available and ~14.4pt line height, we expect roughly 1 line per page.
    // Total 5 lines -> 5 pages.
    println!("Pages generated: {}", pages.len());
    for (i, p) in pages.iter().enumerate() {
        println!("Page {}: {} elements", i+1, p.len());
        for el in p {
            if let LayoutElement::Text(t) = &el.element {
                println!(" - '{}' at y={}", t.content, el.y);
            }
        }
    }

    // If the bug exists (loop or bad split), we might see:
    // 1. Infinite loop (test timeout)
    // 2. Many pages with 1 word each (if the paragraph was wrapping, not explicit newlines).

    // Let's try wrapping text instead of explicit newlines to test width/wrapping split logic.
    // "Word1 Word2 Word3 ..."
}