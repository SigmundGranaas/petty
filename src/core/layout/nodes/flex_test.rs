#![cfg(test)]

use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap};
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use std::collections::HashMap;
use crate::core::idf::{IRNode, NodeMetadata};

fn create_flex_item_with_style(text: &str, style: ElementStyle) -> IRNode {
    IRNode::Block {
        meta: NodeMetadata {
            style_override: Some(style),
            ..Default::default()
        },
        children: vec![create_paragraph(text)],
    }
}

fn get_stylesheet(width: f32, height: f32) -> Stylesheet {
    Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width, height },
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    }
}

#[test]
fn test_flex_direction_row() {
    let stylesheet = get_stylesheet(500.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: Default::default(),
        children: vec![
            create_flex_item_with_style("1", ElementStyle { width: Some(Dimension::Pt(100.0)), ..Default::default() }),
            create_flex_item_with_style("2", ElementStyle { width: Some(Dimension::Pt(100.0)), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item1 = find_first_text_box_with_content(&pages[0], "1").unwrap();
    let item2 = find_first_text_box_with_content(&pages[0], "2").unwrap();
    assert_eq!(item1.x, 0.0);
    assert_eq!(item2.x, 100.0);
}

#[test]
fn test_flex_direction_column() {
    let stylesheet = get_stylesheet(100.0, 500.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: NodeMetadata {
            style_override: Some(ElementStyle { flex_direction: Some(FlexDirection::Column), ..Default::default() }),
            ..Default::default()
        },
        children: vec![
            create_flex_item_with_style("1", ElementStyle { height: Some(Dimension::Pt(100.0)), ..Default::default() }),
            create_flex_item_with_style("2", ElementStyle { height: Some(Dimension::Pt(100.0)), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item1 = find_first_text_box_with_content(&pages[0], "1").unwrap();
    let item2 = find_first_text_box_with_content(&pages[0], "2").unwrap();
    assert_eq!(item1.y, 0.0);
    assert_eq!(item2.y, 100.0);
}

#[test]
fn test_flex_direction_row_reverse() {
    let stylesheet = get_stylesheet(500.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: NodeMetadata {
            style_override: Some(ElementStyle { flex_direction: Some(FlexDirection::RowReverse), ..Default::default() }),
            ..Default::default()
        },
        children: vec![
            create_flex_item_with_style("1", ElementStyle { width: Some(Dimension::Pt(100.0)), ..Default::default() }),
            create_flex_item_with_style("2", ElementStyle { width: Some(Dimension::Pt(100.0)), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item1 = find_first_text_box_with_content(&pages[0], "1").unwrap();
    let item2 = find_first_text_box_with_content(&pages[0], "2").unwrap();
    // Free space is 300. With row-reverse and justify-start, the block is pushed to the right.
    // The items are reversed, so '2' then '1' are laid out LTR inside the right-aligned block.
    // Block starts at x=300. Item '2' is at x=300. Item '1' is at x=400.
    assert!((item2.x - 300.0).abs() < 0.1, "Item 2 should be at x=300");
    assert!((item1.x - 400.0).abs() < 0.1, "Item 1 should be at x=400");
}

#[test]
fn test_flex_grow() {
    let stylesheet = get_stylesheet(500.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: Default::default(),
        children: vec![
            create_flex_item_with_style("1", ElementStyle { width: Some(Dimension::Pt(100.0)), flex_grow: Some(1.0), ..Default::default() }),
            create_flex_item_with_style("2", ElementStyle { width: Some(Dimension::Pt(100.0)), flex_grow: Some(3.0), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item1 = find_first_text_box_with_content(&pages[0], "1").unwrap();
    let item2 = find_first_text_box_with_content(&pages[0], "2").unwrap();
    // Free space = 500 - 200 = 300. Total grow = 4.
    // Item 1 grows by 1/4 * 300 = 75. New width = 175.
    // Item 2 starts after item 1.
    assert_eq!(item1.x, 0.0);
    assert!((item2.x - 175.0).abs() < 0.1, "Item 2 should start at x=175");
}

#[test]
fn test_flex_shrink() {
    let stylesheet = get_stylesheet(400.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: Default::default(),
        children: vec![
            create_flex_item_with_style("1", ElementStyle { flex_basis: Some(Dimension::Pt(300.0)), flex_shrink: Some(1.0), ..Default::default() }),
            create_flex_item_with_style("2", ElementStyle { flex_basis: Some(Dimension::Pt(300.0)), flex_shrink: Some(1.0), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item1 = find_first_text_box_with_content(&pages[0], "1").unwrap();
    let item2 = find_first_text_box_with_content(&pages[0], "2").unwrap();
    // Overflow = 600 - 400 = 200. Total shrink*basis = 300+300=600.
    // Each item shrinks by (300/600) * 200 = 100. New width = 200.
    // Item 2 starts after item 1.
    assert_eq!(item1.x, 0.0);
    assert!((item2.x - 200.0).abs() < 0.1, "Item 2 should start at x=200");
}

#[test]
fn test_order_property() {
    let stylesheet = get_stylesheet(500.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: Default::default(),
        children: vec![
            create_flex_item_with_style("A", ElementStyle { width: Some(Dimension::Pt(100.0)), order: Some(2), ..Default::default() }),
            create_flex_item_with_style("B", ElementStyle { width: Some(Dimension::Pt(100.0)), order: Some(1), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item_a = find_first_text_box_with_content(&pages[0], "A").unwrap();
    let item_b = find_first_text_box_with_content(&pages[0], "B").unwrap();
    // B has lower order, so it comes first.
    assert_eq!(item_b.x, 0.0);
    assert_eq!(item_a.x, 100.0);
}

#[test]
fn test_margins_on_flex_items() {
    let stylesheet = get_stylesheet(500.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: Default::default(),
        children: vec![
            create_flex_item_with_style("1", ElementStyle { width: Some(Dimension::Pt(100.0)), margin: Some(Margins { left: 20.0, ..Default::default() }), ..Default::default() }),
            create_flex_item_with_style("2", ElementStyle { width: Some(Dimension::Pt(100.0)), margin: Some(Margins { left: 30.0, ..Default::default() }), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item1 = find_first_text_box_with_content(&pages[0], "1").unwrap();
    let item2 = find_first_text_box_with_content(&pages[0], "2").unwrap();
    // Item 1 starts after its 20pt left margin.
    assert_eq!(item1.x, 20.0);
    // Item 2 starts after item 1 (100pt) + item 1's margin (20pt) + item 2's margin (30pt).
    assert_eq!(item2.x, 20.0 + 100.0 + 30.0);
}

#[test]
fn test_align_items_center() {
    let stylesheet = get_stylesheet(500.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: NodeMetadata {
            style_override: Some(ElementStyle { align_items: Some(AlignItems::Center), ..Default::default() }),
            ..Default::default()
        },
        children: vec![
            create_flex_item_with_style("1", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(40.0)), ..Default::default() }),
            create_flex_item_with_style("2", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(60.0)), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let item1 = find_first_text_box_with_content(&pages[0], "1").unwrap();
    // Line cross size is max height = 60.
    // Item 1 is 40 high. Centered y = (60 - 40) / 2 = 10.
    assert!((item1.y - 10.0).abs() < 0.1);
}

#[test]
fn test_align_self_override() {
    let stylesheet = get_stylesheet(500.0, 100.0);
    let nodes = vec![IRNode::FlexContainer {
        meta: NodeMetadata {
            style_override: Some(ElementStyle { height: Some(Dimension::Pt(100.0)), align_items: Some(AlignItems::FlexStart), ..Default::default() }),
            ..Default::default()
        },
        children: vec![
            create_flex_item_with_style("tall", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(80.0)), ..Default::default() }),
            create_flex_item_with_style("short", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(20.0)), align_self: Some(AlignSelf::FlexEnd), ..Default::default() }),
        ],
    }];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let tall = find_first_text_box_with_content(&pages[0], "tall").unwrap();
    let short = find_first_text_box_with_content(&pages[0], "short").unwrap();
    // Line cross size is the height of the container (100) because it's single line flex and height is definite.
    assert!((tall.y - 0.0).abs() < 0.1);
    // Short item (20h) aligns to the end of the line box (100h). Its y position should be 100 - 20 = 80.
    // Updated expectation to match standard Flexbox behavior (and Taffy implementation).
    assert!((short.y - 80.0).abs() < 0.1);
}


#[test]
fn test_flex_wrap_with_page_break() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 320.0, height: 80.0 },
                margins: Some(Margins::all(10.0)), // content 300w, 60h
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let nodes = vec![IRNode::FlexContainer {
        meta: NodeMetadata {
            style_override: Some(ElementStyle { flex_wrap: Some(FlexWrap::Wrap), ..Default::default() }),
            ..Default::default()
        },
        children: vec![
            create_flex_item_with_style("1", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(40.0)), ..Default::default() }), // Line 1
            create_flex_item_with_style("2", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(40.0)), ..Default::default() }),
            create_flex_item_with_style("3", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(40.0)), ..Default::default() }),
            create_flex_item_with_style("4", ElementStyle { width: Some(Dimension::Pt(100.0)), height: Some(Dimension::Pt(40.0)), ..Default::default() }), // Line 2, needs 40h, available is 20h.
        ],
    }];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    assert_eq!(pages.len(), 2);
    let page1 = &pages[0];
    let page2 = &pages[1];

    assert!(find_first_text_box_with_content(page1, "3").is_some());

    // Item 4 overflows page 1 (available 20h, needs 40h).
    // FlexNode pushes non-first items to next page.
    // So Item 4 should be on Page 2.
    assert!(find_first_text_box_with_content(page1, "4").is_none(), "Item 4 should be pushed to page 2");
    assert!(find_first_text_box_with_content(page2, "4").is_some(), "Item 4 should be on page 2");
}

#[test]
fn test_block_split_resets_padding_and_border() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 100.0 }, // 100 height
                margins: Some(Margins::all(10.0)), // Content height 80
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let block_style = ElementStyle {
        padding: Some(Margins { top: 20.0, ..Default::default() }),
        border_top: Some(crate::core::style::border::Border {
            width: 10.0,
            style: crate::core::style::border::BorderStyle::Solid,
            color: Default::default(),
        }),
        ..Default::default()
    };

    let nodes = vec![IRNode::Block {
        meta: NodeMetadata {
            style_override: Some(block_style),
            ..Default::default()
        },
        children: vec![
            create_paragraph("Child 1"), // ~14.4px
            create_paragraph("Child 2\nLine 2\nLine 3\nLine 4"), // Taller to force push/split
        ],
    }];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    assert_eq!(pages.len(), 2);

    let page2 = &pages[1];
    let text_on_p2 = find_first_text_box_with_content(page2, "Line").unwrap();

    // If bug exists, y will be > 10.0 (e.g. 10 + 10 + 20 = 40).
    // If fixed, y should be 10.0 (plus maybe small margin if paragraph has one, default is 0).
    assert!((text_on_p2.y - 10.0).abs() < 1.0, "Content on page 2 should start at top margin, got {}", text_on_p2.y);
}