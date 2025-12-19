#![cfg(test)]

use crate::core::idf::{IRNode, NodeMetadata};
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::layout::LayoutElement;
use crate::core::style::border::{Border, BorderStyle};
use crate::core::base::color::Color;
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
        meta: NodeMetadata {
            style_override: Some(block_style),
            ..Default::default()
        },
        children: vec![create_paragraph("Indented text.")],
    }];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];
    let text_el = find_first_text_box_with_content(page1, "Indented").unwrap();

    // Expected X = page margin (10) + block padding (25) = 35
    assert_eq!(text_el.x, 35.0);
}

#[test]
fn test_block_with_border_and_padding_indents_content() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 500.0 },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let block_style = ElementStyle {
        padding: Some(Margins::all(20.0)),
        border: Some(Border {
            width: 5.0,
            style: BorderStyle::Solid,
            color: Color::default(),
        }),
        ..Default::default()
    };
    let nodes = vec![IRNode::Block {
        meta: NodeMetadata {
            style_override: Some(block_style),
            ..Default::default()
        },
        children: vec![create_paragraph("Content")],
    }];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];
    let text_el = find_first_text_box_with_content(page1, "Content").unwrap();

    // Expected position = page_margin (10) + border_width (5) + padding (20)
    let expected_pos = 10.0 + 5.0 + 20.0;
    assert!((text_el.x - expected_pos).abs() < 0.1);
    assert!((text_el.y - expected_pos).abs() < 0.1);

    // Verify 4 border rectangles were drawn
    let border_rects = page1
        .iter()
        .filter(|el| matches!(el.element, LayoutElement::Rectangle(_)))
        .count();
    assert_eq!(border_rects, 4);
}

#[test]
fn test_multipage_block_with_background_is_drawn_on_all_pages() {
    // Page content height = 80. Line height = 14.4. 5 lines fit.
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 100.0 },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let block_style_override = ElementStyle {
        widows: Some(1),
        background_color: Some(Color { r: 255, g: 0, b: 0, a: 1.0 }),
        ..Default::default()
    };
    let nodes = vec![IRNode::Block {
        meta: NodeMetadata {
            style_override: Some(block_style_override),
            ..Default::default()
        },
        children: vec![
            create_paragraph("Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6"), // 6 lines
        ],
    }];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    assert_eq!(pages.len(), 2, "Expected two pages");

    let page1_bg = pages[0]
        .iter()
        .find(|el| matches!(el.element, LayoutElement::Rectangle(_)))
        .expect("Background rectangle not found on page 1");
    let page2_bg = pages[1]
        .iter()
        .find(|el| matches!(el.element, LayoutElement::Rectangle(_)))
        .expect("Background rectangle not found on page 2");

    // Page 1 has 5 lines. Height should be 5 * 14.4 = 72.0
    assert!((page1_bg.height - 72.0).abs() < 0.1);
    assert_eq!(page1_bg.y, 10.0);

    // Page 2 has 1 line. Height should be 14.4
    assert!((page2_bg.height - 14.4).abs() < 0.1);
    assert_eq!(page2_bg.y, 10.0);
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

    // Default widow control (2) would move an extra line to the next page, making this test fail.
    // We create a style to set widows to 1, allowing a single line ("widow") on the next page,
    // which is the behavior this test is designed to verify.
    let block_style_override = ElementStyle {
        widows: Some(1),
        ..Default::default()
    };
    let nodes = vec![IRNode::Block {
        meta: NodeMetadata {
            style_override: Some(block_style_override),
            ..Default::default()
        },
        children: vec![
            create_paragraph("Line 1\nLine 2\nLine 3"), // 3 lines
            create_paragraph("Line 4\nLine 5\nLine 6"), // 3 lines
        ],
    }];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();

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

#[test]
fn test_vertical_margin_collapse() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 500.0 },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let block_style_1 = ElementStyle {
        margin: Some(Margins { bottom: 20.0, ..Default::default() }),
        ..Default::default()
    };
    let block_style_2 = ElementStyle {
        margin: Some(Margins { top: 30.0, ..Default::default() }),
        ..Default::default()
    };

    let nodes = vec![
        IRNode::Block {
            meta: NodeMetadata {
                style_override: Some(block_style_1),
                ..Default::default()
            },
            children: vec![create_paragraph("Block 1")],
        },
        IRNode::Block {
            meta: NodeMetadata {
                style_override: Some(block_style_2),
                ..Default::default()
            },
            children: vec![create_paragraph("Block 2")],
        },
    ];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    let text1 = find_first_text_box_with_content(page1, "Block 1").unwrap();
    let text2 = find_first_text_box_with_content(page1, "Block 2").unwrap();

    // Text1 is at page margin top
    assert!((text1.y - 10.0).abs() < 0.1);

    // Text2 should be below Text1.
    // Y position = text1.y + text1.height + collapsed_margin
    // Collapsed margin should be max(20.0, 30.0) = 30.0
    // Default line height is 14.4
    let expected_y2 = 10.0 + 14.4 + 30.0;

    assert!(
        (text2.y - expected_y2).abs() < 0.1,
        "Margins did not collapse correctly. Expected y={}, got {}.",
        expected_y2,
        text2.y
    );
}