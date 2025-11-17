// src/core/layout/nodes/table_test.rs
#![cfg(test)]

use crate::core::idf::{IRNode, TableBody, TableCell, TableColumnDefinition, TableHeader, TableRow};
use crate::core::layout::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::stylesheet::{PageLayout, Stylesheet};
use std::collections::HashMap;

fn get_stylesheet(width: f32, height: f32) -> Stylesheet {
    Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom { width, height },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    }
}

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
        meta: Default::default(),
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
    // Page: 520w, margin 10. Table content width = 500. Cols will be 250w.
    let stylesheet = get_stylesheet(520.0, 500.0);
    let table = create_test_table(2);
    let nodes = vec![table];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    let h1 = find_first_text_box_with_content(page1, "Header 1").unwrap();
    let h2 = find_first_text_box_with_content(page1, "Header 2").unwrap();
    let r1c1 = find_first_text_box_with_content(page1, "R1C1").unwrap();

    assert_eq!(h1.x, 10.0); // Starts at page margin
    assert_eq!(h1.y, 10.0);
    assert!((h2.x - (10.0 + 250.0)).abs() < 1.0);
    assert_eq!(h2.y, 10.0);
    assert!((r1c1.y - (10.0 + 14.4)).abs() < 0.1);
}

#[test]
fn test_table_splits_across_pages_and_repeats_header() {
    // Page content height = 50. Line height = 14.4.
    // Can fit Header (14.4) + Row 1 (14.4) + Row 2 (14.4) = 43.2.
    // Row 3 starts at 43.2, needs 14.4, bottom would be 57.6, which > 50.
    // So, page break before Row 3.
    let stylesheet = get_stylesheet(520.0, 70.0); // content height = 50
    let table = create_test_table(5);
    let nodes = vec![table];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    assert_eq!(pages.len(), 3, "Expected table to split into 3 pages");

    let page1 = &pages[0];
    assert!(find_first_text_box_with_content(page1, "Header 1").is_some());
    assert!(find_first_text_box_with_content(page1, "R2C1").is_some());
    assert!(find_first_text_box_with_content(page1, "R3C1").is_none());

    let page2 = &pages[1];
    assert!(find_first_text_box_with_content(page2, "Header 1").is_some(), "Header should repeat on page 2");
    let r3c1 = find_first_text_box_with_content(page2, "R3C1").unwrap();
    assert!((r3c1.y - (10.0 + 14.4)).abs() < 0.1, "R3C1 should appear after repeated header");
}


#[test]
fn test_table_colspan_and_rowspan() {
    let stylesheet = get_stylesheet(520.0, 500.0); // content width 500
    let table = IRNode::Table {
        meta: Default::default(),
        columns: vec![
            TableColumnDefinition { width: Some(Dimension::Pt(100.0)), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Pt(100.0)), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Pt(100.0)), ..Default::default() },
        ],
        header: None,
        body: Box::new(TableBody { rows: vec![
            // Row 1
            TableRow { cells: vec![
                TableCell { rowspan: 2, children: vec![create_paragraph("A")], ..Default::default() },
                TableCell { colspan: 2, children: vec![create_paragraph("B")], ..Default::default() },
            ]},
            // Row 2
            TableRow { cells: vec![
                // Cell "A" from row 1 occupies this slot
                TableCell { children: vec![create_paragraph("C")], ..Default::default() },
                TableCell { children: vec![create_paragraph("D")], ..Default::default() },
            ]},
            // Row 3
            TableRow { cells: vec![
                TableCell { children: vec![create_paragraph("E")], ..Default::default() },
                TableCell { children: vec![create_paragraph("F")], ..Default::default() },
                TableCell { children: vec![create_paragraph("G")], ..Default::default() },
            ]}
        ]}),
    };
    let (pages, _, _) = paginate_test_nodes(stylesheet, vec![table]).unwrap();
    let page1 = &pages[0];

    let cell_a = find_first_text_box_with_content(page1, "A").unwrap();
    let cell_b = find_first_text_box_with_content(page1, "B").unwrap();
    let cell_c = find_first_text_box_with_content(page1, "C").unwrap();
    let cell_d = find_first_text_box_with_content(page1, "D").unwrap();
    let cell_e = find_first_text_box_with_content(page1, "E").unwrap();

    // Row 1
    assert!((cell_a.x - 10.0).abs() < 0.1);
    assert!((cell_a.y - 10.0).abs() < 0.1);
    assert!((cell_b.x - (10.0 + 100.0)).abs() < 0.1, "B should be in the second column");

    // Row 2
    // C should be in the second column because A (rowspan=2) is in the first
    assert!((cell_c.x - (10.0 + 100.0)).abs() < 0.1);
    assert!((cell_c.y - (10.0 + 14.4)).abs() < 0.1, "C should be on the second line");
    assert!((cell_d.x - (10.0 + 200.0)).abs() < 0.1);

    // Row 3
    assert!((cell_e.x - 10.0).abs() < 0.1, "E should be back in the first column");
    assert!((cell_e.y - (10.0 + 14.4 * 2.0)).abs() < 0.1, "E should be on the third line");
}