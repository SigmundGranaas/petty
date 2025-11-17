// src/core/layout/nodes/image_test.rs
#![cfg(test)]
use crate::core::idf::{IRNode, NodeMetadata};
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use std::collections::HashMap;

fn create_image(height: f32) -> IRNode {
    IRNode::Image {
        src: "test.png".to_string(),
        meta: NodeMetadata {
            style_override: Some(ElementStyle {
                height: Some(Dimension::Pt(height)),
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}

#[test]
fn test_image_splits_to_next_page() {
    // Page content height = 80.
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
    let nodes = vec![
        create_image(40.0), // Fits
        create_image(50.0), // Does not fit (40 + 50 > 80)
    ];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();

    assert_eq!(pages.len(), 2, "Expected two pages");

    let page1 = &pages[0];
    assert_eq!(page1.len(), 1);
    assert_eq!(page1[0].y, 10.0);
    assert_eq!(page1[0].height, 40.0);

    let page2 = &pages[1];
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].y, 10.0); // Starts at top of new page
    assert_eq!(page2[0].height, 50.0);
}

#[test]
fn test_image_with_margins() {
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
    let image_style = ElementStyle {
        height: Some(Dimension::Pt(30.0)),
        margin: Some(Margins { top: 15.0, bottom: 5.0, ..Default::default() }),
        ..Default::default()
    };
    let nodes = vec![
        IRNode::Image {
            src: "test.png".to_string(),
            meta: NodeMetadata {
                style_override: Some(image_style),
                ..Default::default()
            },
        }, // Total height = 15+30+5 = 50
        create_image(20.0), // Starts at y=10+50. Fits (10+50+20 <= 80)
    ];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    assert_eq!(pages.len(), 1);

    let page1 = &pages[0];
    assert_eq!(page1.len(), 2);

    let img1 = &page1[0];
    // Y position = page_margin (10) + image_margin (15) = 25
    assert_eq!(img1.y, 25.0);

    let img2 = &page1[1];
    // Y position = img1_start (10) + img1_total_height (50) = 60
    assert_eq!(img2.y, 60.0);
}

#[test]
fn test_image_taller_than_page_is_skipped() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 100.0 },
                margins: Some(Margins::all(10.0)), // Content height = 80
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let nodes = vec![
        create_paragraph("Before"),
        create_image(90.0), // Taller than content height
        create_paragraph("After"),
    ];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();

    assert_eq!(pages.len(), 1, "Should only produce one page");
    let page1 = &pages[0];
    // The oversized image is skipped, but the other paragraphs are rendered.
    assert_eq!(page1.len(), 2);
    assert!(find_first_text_box_with_content(page1, "Before").is_some());
    assert!(find_first_text_box_with_content(page1, "After").is_some());
}