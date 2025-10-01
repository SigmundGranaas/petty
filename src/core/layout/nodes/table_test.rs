// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/table_test.rs
#![cfg(test)]

use crate::core::idf::{IRNode, TableBody, TableCell, TableColumnDefinition, TableHeader, TableRow};
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::stylesheet::{PageLayout, Stylesheet};
use std::collections::HashMap;

fn create_test_table(rows: usize) -> IRNode {
    let mut body_rows = Vec::new();
    for i in 1..=rows {
        body_rows.push(TableRow {
            cells: vec![
                TableCell {
                    children: vec![create_paragraph(&format!("R{}C1", i))],
                    ..Default::default()
                },
                TableCell {
                    children: vec![create_paragraph(&format!("R{}C2", i))],
                    ..Default::default()
                },
            ],
        });
    }

    IRNode::Table {
        style_sets: vec![],
        style_override: None,
        columns: vec![
            TableColumnDefinition {
                width: Some(Dimension::Percent(50.0)),
                ..Default::default()
            },
            TableColumnDefinition {
                width: Some(Dimension::Percent(50.0)),
                ..Default::default()
            },
        ],
        header: Some(Box::new(TableHeader {
            rows: vec![TableRow {
                cells: vec![
                    TableCell {
                        children: vec![create_paragraph("Header 1")],
                        ..Default::default()
                    },
                    TableCell {
                        children: vec![create_paragraph("Header 2")],
                        ..Default::default()
                    },
                ],
            }],
        })),
        body: Box::new(TableBody { rows: body_rows }),
    }
}

#[test]
fn test_table_basic_layout() {
    // Page: 500w. Table will use all of it. Cols will be 250w.
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 500.0 },
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let table = create_test_table(2);
    let nodes = vec![table];

    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    // Header(2) + R1(2) + R2(2) = 6 text elements
    assert_eq!(page1.len(), 6);

    let h1 = find_first_text_box_with_content(page1, "Header 1").unwrap();
    let h2 = find_first_text_box_with_content(page1, "Header 2").unwrap();
    let r1c1 = find_first_text_box_with_content(page1, "R1C1").unwrap();

    assert_eq!(h1.x, 0.0); // No page margin
    assert_eq!(h1.y, 0.0);
    // The positioned element's width is the width of the text itself, not the cell.
    // So we verify the column width by checking the position of the next column's content.
    assert!(h1.width < 250.0);

    assert!((h2.x - 250.0).abs() < 1.0); // Column 2 starts after col 1 width
    assert_eq!(h2.y, 0.0);

    // Default line height is 14.4
    assert!((r1c1.y - 14.4).abs() < 0.1); // Row 1 starts after header row
}

#[test]
fn test_table_splits_across_pages() {
    // Page content height = 50. Line height = 14.4.
    // Can fit Header (14.4) + Row 1 (14.4) + Row 2 (14.4) = 43.2.
    // Row 3 starts at 43.2, needs 14.4, bottom would be 57.6, which > 50.
    // So, page break before Row 3.
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width: 500.0, height: 70.0 },
                margins: Some(Margins::all(10.0)), // Content height = 50
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let table = create_test_table(5); // Header + 5 rows
    let nodes = vec![table];

    let pages = paginate_test_nodes(stylesheet, nodes).unwrap();
    assert_eq!(pages.len(), 2, "Expected table to split into 2 pages");

    // Page 1: Header + 2 Rows = 3*2 = 6 elements
    let page1 = &pages[0];
    assert_eq!(page1.len(), 6);
    assert!(find_first_text_box_with_content(page1, "Header 1").is_some());
    assert!(find_first_text_box_with_content(page1, "R2C1").is_some());
    assert!(find_first_text_box_with_content(page1, "R3C1").is_none());

    // Page 2: Rows 3, 4, 5 = 3*2 = 6 elements
    let page2 = &pages[1];
    assert_eq!(page2.len(), 6);
    assert!(find_first_text_box_with_content(page2, "R2C1").is_none());
    let r3c1 = find_first_text_box_with_content(page2, "R3C1").unwrap();
    assert!(find_first_text_box_with_content(page2, "R5C2").is_some());
    assert_eq!(r3c1.y, 10.0); // Starts at top of new page
}