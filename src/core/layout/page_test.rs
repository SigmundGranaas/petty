// FILE: src/core/layout/page_test.rs

use super::test_utils::{create_layout_unit, create_paragraph, create_test_engine_with_page};
use crate::core::idf::{
    IRNode, InlineNode, TableBody, TableCell, TableColumnDefinition, TableRow,
};
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::font::FontWeight;
use crate::core::style::stylesheet::ElementStyle;
use std::sync::Arc;

#[test]
fn test_simple_page_break() {
    // A page that can hold ~10 lines. We will add 20.
    // Page height 200, margin 10. Content height = 180.
    // Line height 14.4. 180 / 14.4 = 12.5 lines.
    let engine = create_test_engine_with_page(500.0, 200.0, 10.0);
    let top_margin = engine.stylesheet.page.margins.top;

    let mut children = Vec::new();
    for i in 0..20 {
        children.push(create_paragraph(&format!("Line {}", i + 1)));
    }
    let tree = IRNode::Root(children);
    let layout_unit = create_layout_unit(tree);

    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();

    // Page 1
    let page1 = page_iter.next().expect("Should have a first page");
    assert!(!page1.is_empty(), "Page 1 should not be empty");
    assert_eq!(page1.first().unwrap().y, top_margin); // Starts at top margin
    assert!(page1.len() > 5 && page1.len() < 15, "Page 1 has an unexpected number of lines");

    // Page 2
    let page2 = page_iter.next().expect("Should have a second page");
    assert!(!page2.is_empty(), "Page 2 should not be empty");
    assert_eq!(
        page2.first().unwrap().y,
        top_margin,
        "Page 2 should start at the top margin"
    );

    // No more pages
    assert!(page_iter.next().is_none(), "Should only have two pages");
}

#[test]
fn test_table_pagination() {
    let engine = create_test_engine_with_page(600.0, 300.0, 50.0);
    let top_margin = engine.stylesheet.page.margins.top;

    let mut rows = Vec::new();
    for i in 0..20 {
        rows.push(TableRow {
            cells: vec![TableCell {
                style_sets: vec![],
                style_override: None,
                children: vec![create_paragraph(&format!("Row {}", i + 1))],
            }],
        });
    }

    let table = IRNode::Table {
        style_sets: vec![],
        style_override: None,
        columns: vec![TableColumnDefinition::default()],
        calculated_widths: vec![], // Will be calculated by engine
        header: None,
        body: Box::new(TableBody { rows }),
    };

    let layout_unit = create_layout_unit(IRNode::Root(vec![table]));
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();

    let page1 = page_iter.next().expect("Should get page 1");
    assert!(!page1.is_empty());
    assert_eq!(page1[0].y, top_margin); // Table starts at top margin
    let page1_rows = page1.len();

    let page2 = page_iter.next().expect("Should get page 2");
    assert!(!page2.is_empty());
    assert_eq!(page2[0].y, top_margin); // Remainder of table starts at top margin on new page
    let page2_rows = page2.len();

    assert_eq!(page1_rows + page2_rows, 20);
    assert!(page_iter.next().is_none());
}

#[test]
fn test_element_taller_than_page_is_skipped() {
    let engine = create_test_engine_with_page(500.0, 200.0, 10.0);
    let page_content_height = 180.0;

    let too_tall_block = IRNode::Block {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            height: Some(Dimension::Pt(page_content_height + 1.0)),
            ..Default::default()
        }),
        children: vec![create_paragraph("This block is too tall.")],
    };

    let following_paragraph = create_paragraph("This paragraph should appear on page 1.");

    let tree = IRNode::Root(vec![too_tall_block, following_paragraph]);
    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();

    let page1 = page_iter.next().expect("Should have one page");
    // The tall block should be skipped, and only the following paragraph rendered.
    assert_eq!(page1.len(), 1, "Only the second, valid paragraph should be rendered");
    // Check that the rendered element is indeed the second one.
    if let super::LayoutElement::Text(text_el) = &page1[0].element {
        assert!(text_el.content.contains("should appear"));
    } else {
        panic!("Expected a text element");
    }

    assert!(page_iter.next().is_none(), "Should be no second page");
}

#[test]
fn test_flex_container_vertical_stacking() {
    let engine = create_test_engine_with_page(600.0, 800.0, 25.0);

    let tree = IRNode::Root(vec![
        IRNode::FlexContainer {
            style_sets: vec![],
            style_override: None,
            children: vec![
                // Left block (flex item 1)
                IRNode::Block {
                    style_sets: vec![],
                    style_override: Some(ElementStyle {
                        width: Some(Dimension::Percent(50.0)),
                        ..Default::default()
                    }),
                    children: vec![create_paragraph("Left Column")],
                },
                // Right block (flex item 2)
                IRNode::Block {
                    style_sets: vec![],
                    style_override: Some(ElementStyle {
                        width: Some(Dimension::Percent(50.0)),
                        ..Default::default()
                    }),
                    children: vec![
                        create_paragraph("Right Column Line 1"),
                        create_paragraph("Right Column Line 2"),
                    ],
                },
            ],
        }
    ]);

    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
    let page = page_iter.next().expect("Should produce one page");

    assert_eq!(page.len(), 3, "Should be 3 text elements total");

    let left_el = &page[0];
    let right_el_1 = &page[1];
    let right_el_2 = &page[2];

    let top_margin = engine.stylesheet.page.margins.top;

    // Check left element
    assert!((left_el.y - top_margin).abs() < 0.01); // At top margin

    // Check right elements are stacked vertically
    assert!((right_el_1.y - top_margin).abs() < 0.01, "First right line should be at the top");
    assert!(right_el_2.y > right_el_1.y, "Second right line should be below the first. Y1: {}, Y2: {}", right_el_1.y, right_el_2.y);

    let line_height = 14.4; // Default from ComputedStyle
    assert!((right_el_2.y - right_el_1.y - line_height).abs() < 0.01, "Vertical spacing should be one line height");

    // Check right elements are horizontally offset
    let page_content_width = 600.0 - 25.0 - 25.0;
    let halfway_point = 25.0 + page_content_width * 0.5;
    assert!(right_el_1.x >= halfway_point, "Right elements should start in the second half of the page. Got X: {}", right_el_1.x);
    assert!(right_el_2.x >= halfway_point, "Right elements should start in the second half of the page. Got X: {}", right_el_2.x);
}

#[test]
fn test_nested_block_indentation() {
    let engine = create_test_engine_with_page(600.0, 800.0, 50.0);
    let page_margin = 50.0;

    let tree = IRNode::Root(vec![
        IRNode::Block { // Parent block
            style_sets: vec![],
            style_override: Some(ElementStyle {
                margin: Some(Margins { left: 20.0, ..Default::default() }),
                padding: Some(Margins { left: 10.0, ..Default::default() }),
                ..Default::default()
            }),
            children: vec![
                IRNode::Block { // Child block
                    style_sets: vec![],
                    style_override: Some(ElementStyle {
                        margin: Some(Margins { left: 15.0, ..Default::default() }),
                        padding: Some(Margins { left: 5.0, ..Default::default() }),
                        ..Default::default()
                    }),
                    children: vec![
                        create_paragraph("Deeply nested text.")
                    ]
                }
            ],
        }
    ]);

    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
    let page = page_iter.next().expect("Should produce one page");

    assert_eq!(page.len(), 1, "Should be 1 text element");
    let text_el = &page[0];

    // Calculate expected X position
    // page_margin (50) + parent_margin (20) + parent_padding (10)
    // + child_margin (15) + child_padding (5)
    let expected_x = page_margin + 20.0 + 10.0 + 15.0 + 5.0; // 100.0

    assert!((text_el.x - expected_x).abs() < 0.01, "Text should be indented by all parent margins and paddings. Expected {}, got {}", expected_x, text_el.x);
}

// --- NEW TESTS FOR BUG FIXES ---

#[test]
fn test_job_title_and_company_on_same_line() {
    let engine = create_test_engine_with_page(600.0, 800.0, 25.0);

    // This IR structure simulates the output of the *corrected* XSLT.
    let tree = IRNode::Root(vec![IRNode::Paragraph {
        style_sets: vec![],
        style_override: None,
        children: vec![
            InlineNode::StyledSpan {
                style_sets: vec![Arc::new(ElementStyle {
                    font_weight: Some(FontWeight::Bold),
                    ..Default::default()
                })],
                style_override: None,
                children: vec![InlineNode::Text("Senior Software Engineer".to_string())],
            },
            InlineNode::Text(" at ".to_string()),
            InlineNode::Text("Innovatech Solutions Inc.".to_string()),
        ],
    }]);

    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
    let page = page_iter.next().expect("Should produce one page");

    // We expect three text elements for the three inline nodes.
    assert_eq!(page.len(), 3, "Expected 3 text elements for the single paragraph");

    let first_el_y = page[0].y;
    // All elements should be on the same line (same Y coordinate).
    assert!((page[1].y - first_el_y).abs() < 0.01, "Text 'at' should be on the same line");
    assert!((page[2].y - first_el_y).abs() < 0.01, "Company name should be on the same line");
}

#[test]
fn test_contact_info_line_height() {
    let engine = create_test_engine_with_page(600.0, 800.0, 25.0);
    let line_height = 14.0; // From contact-info style in XSLT

    // Simulate the corrected IR: one paragraph with text and line breaks.
    let tree = IRNode::Root(vec![IRNode::Paragraph {
        style_sets: vec![],
        style_override: Some(ElementStyle { line_height: Some(line_height), ..Default::default() }),
        children: vec![
            InlineNode::Text("alex.doe@example.com".to_string()),
            InlineNode::LineBreak,
            InlineNode::Text("+1 (555) 123-4567".to_string()),
        ],
    }]);

    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
    let page = page_iter.next().expect("Should produce one page");

    assert_eq!(page.len(), 2, "Expected 2 text elements");

    let email_el = &page[0];
    let phone_el = &page[1];

    // The vertical distance between the start of the lines should be exactly one line_height.
    let y_diff = phone_el.y - email_el.y;
    assert!((y_diff - line_height).abs() < 0.01, "Spacing between lines should be exactly line_height. Expected {}, got {}", line_height, y_diff);
}

#[test]
fn test_bold_style_is_computed() {
    let engine = create_test_engine_with_page(600.0, 800.0, 25.0);
    // This test verifies that the style computation is correct, even if the font
    // rendering part is not yet implemented.
    let tree = IRNode::Root(vec![IRNode::Paragraph {
        style_sets: vec![],
        style_override: None,
        children: vec![InlineNode::StyledSpan {
            style_sets: vec![Arc::new(ElementStyle {
                font_weight: Some(FontWeight::Bold),
                ..Default::default()
            })],
            style_override: None,
            children: vec![InlineNode::Text("This text should be bold".to_string())],
        }],
    }]);

    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
    let page = page_iter.next().expect("Should produce one page");

    assert_eq!(page.len(), 1);
    let text_el = &page[0];

    // Assert that the final computed style for the element has the bold weight.
    assert_eq!(text_el.style.font_weight, FontWeight::Bold, "The computed style for the text element should be bold.");
}