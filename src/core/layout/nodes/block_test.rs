// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/block_test.rs
#![cfg(test)]

use crate::core::idf::IRNode;
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::style::dimension::{Margins, PageSize};
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use std::collections::HashMap;

#[test]
fn test_block_with_padding_indents_child() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: 500.0,
                },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let block_style = ElementStyle {
        padding: Some(Margins {
            left: 25.0,
            ..Default::default()
        }),
        ..Default::default()
    };
    let nodes = vec![IRNode::Block {
        style_sets: vec![],
        style_override: Some(block_style),
        children: vec![create_paragraph("Indented text.")],
    }];

    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];
    let text_el = find_first_text_box_with_content(page1, "Indented").unwrap();

    // Expected X = page margin (10) + block padding (25) = 35
    assert_eq!(text_el.x, 35.0);
}

#[test]
fn test_block_splits_across_pages() {
    // Page content height = 80. Line height is 14.4. 5 lines fit.
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: 100.0,
                },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let nodes = vec![IRNode::Block {
        style_sets: vec![],
        style_override: None,
        children: vec![
            create_paragraph("Line 1\nLine 2\nLine 3"), // 3 lines
            create_paragraph("Line 4\nLine 5\nLine 6"), // 3 lines
        ],
    }];

    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();

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