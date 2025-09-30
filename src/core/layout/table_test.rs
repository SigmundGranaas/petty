#![cfg(test)]

use crate::core::idf::{
    IRNode, TableBody, TableCell, TableColumnDefinition, TableHeader, TableRow,
};
use crate::core::layout::table::calculate_column_widths;
use crate::core::layout::test_utils::{
    create_layout_unit, create_paragraph, create_test_engine, create_test_engine_with_page,
    find_first_text_box_with_content,
};
use crate::core::style::dimension::Dimension;

#[test]
fn test_calculate_column_widths_mixed() {
    let table_width = 1000.0;
    let columns = vec![
        TableColumnDefinition { // Fixed
            width: Some(Dimension::Pt(200.0)), ..Default::default()
        },
        TableColumnDefinition { // Percent
            width: Some(Dimension::Percent(30.0)), ..Default::default()
        },
        TableColumnDefinition { // Auto
            width: Some(Dimension::Auto), ..Default::default()
        },
        TableColumnDefinition { // Auto (None)
            width: None, ..Default::default()
        },
    ];

    let widths = calculate_column_widths(&columns, table_width);

    assert_eq!(widths[0], 200.0);
    assert_eq!(widths[1], 300.0);
    // Remaining width = 1000 - 200 - 300 = 500. Split between 2 auto columns = 250 each
    assert_eq!(widths[2], 250.0);
    assert_eq!(widths[3], 250.0);
}

fn create_test_table() -> IRNode {
    IRNode::Table {
        style_sets: vec![],
        style_override: None,
        columns: vec![
            TableColumnDefinition { width: Some(Dimension::Pt(100.0)), ..Default::default() },
            TableColumnDefinition { width: Some(Dimension::Pt(200.0)), ..Default::default() },
        ],
        calculated_widths: vec![],
        header: Some(Box::new(TableHeader {
            rows: vec![TableRow {
                cells: vec![
                    TableCell { children: vec![create_paragraph("H1")], ..Default::default() },
                    TableCell { children: vec![create_paragraph("H2")], ..Default::default() },
                ],
            }],
        })),
        body: Box::new(TableBody {
            rows: vec![TableRow {
                cells: vec![
                    TableCell { children: vec![create_paragraph("A1")], ..Default::default() },
                    TableCell { children: vec![create_paragraph("A2 is taller\nand taller")], ..Default::default() },
                ],
            },
                       TableRow {
                           cells: vec![
                               TableCell { children: vec![create_paragraph("B1")], ..Default::default() },
                               TableCell { children: vec![create_paragraph("B2")], ..Default::default() },
                           ],
                       }],
        }),
    }
}

#[test]
fn test_table_layout_structure_and_sizing() {
    let engine = create_test_engine();
    let mut tree = create_test_table();

    // Manually run the measurement pass, as build_layout_tree does not do this itself.
    engine.measurement_pass(&mut tree).unwrap();

    // The available width is larger than the table's intrinsic width.
    let layout_box = engine.build_layout_tree(&mut tree, engine.get_default_style(), (500.0, f32::INFINITY));

    // The table's final box is stretched to 500, but its content (the rows) should not be.
    assert!((layout_box.rect.width - 500.0).abs() < 0.01);

    // Table -> 3 Rows (1 header, 2 body)
    let rows = match &layout_box.content {
        super::LayoutContent::Children(c) => c,
        _ => panic!("Table should have row children")
    };
    assert_eq!(rows.len(), 3);

    let header_row = &rows[0];
    let body_row_1 = &rows[1];

    // Check that the *row's width* is its intrinsic width (100+200), not the available width.
    assert!((header_row.rect.width - 300.0).abs() < 0.01, "Header row width was {}", header_row.rect.width);
    assert!((body_row_1.rect.width - 300.0).abs() < 0.01);

    // Check cells inside the first body row
    let row1_cells = match &body_row_1.content {
        super::LayoutContent::Children(c) => c,
        _ => panic!("Row should have cell children")
    };
    assert_eq!(row1_cells.len(), 2);
    let cell_a1 = &row1_cells[0];
    let cell_a2 = &row1_cells[1];

    // Check uniform row height. Cell A2's content is taller, so cell A1 should be stretched to match.
    assert!((cell_a1.rect.height - cell_a2.rect.height).abs() < 0.01, "Cells in a row must have the same height");
    assert!((body_row_1.rect.height - cell_a2.rect.height).abs() < 0.01);
}

#[test]
fn test_table_pagination() {
    let engine = create_test_engine_with_page(600.0, 100.0, 10.0);
    // Page content height is 80.
    // Row heights: H(14.4), A(28.8), B(14.4), R3(14.4).
    // Cumulative bottom Ys (relative to content top): 14.4, 43.2, 57.6, 72.0. All fit.
    // Row R4 starts at Y=72.0, its bottom is at 86.4. This is > 80, so it breaks.
    // Page 1 should have 4 rows.
    // Element count: H(2) + A(3) + B(2) + R3(2) = 9 elements.
    let mut table = create_test_table();
    if let IRNode::Table { body, .. } = &mut table {
        for i in 3..=6 {
            body.rows.push(TableRow {
                cells: vec![
                    TableCell { children: vec![create_paragraph(&format!("R{}C1", i))], ..Default::default() },
                    TableCell { children: vec![create_paragraph(&format!("R{}C2", i))], ..Default::default() },
                ],
            });
        }
    }
    let layout_unit = create_layout_unit(IRNode::Root(vec![table]));
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();

    // Page 1
    let page1 = page_iter.next().unwrap();
    assert_eq!(page1.len(), 9, "Page 1 should have 4 rows (9 elements)");
    assert!(find_first_text_box_with_content(&page1, "H1").is_some());
    assert!(find_first_text_box_with_content(&page1, "A2 is taller").is_some());
    assert!(find_first_text_box_with_content(&page1, "B1").is_some());
    assert!(find_first_text_box_with_content(&page1, "R3C1").is_some());
    assert!(find_first_text_box_with_content(&page1, "R4C1").is_none());

    // Page 2
    let page2 = page_iter.next().unwrap();
    // Remaining rows are R4, R5, R6. That's 3 rows.
    // Element count: R4(2) + R5(2) + R6(2) = 6 elements.
    assert_eq!(page2.len(), 6, "Page 2 should have the remaining 3 rows (6 elements)");
    let r4c1 = find_first_text_box_with_content(&page2, "R4C1").unwrap();
    assert!(find_first_text_box_with_content(&page2, "R5C1").is_some());
    assert!(find_first_text_box_with_content(&page2, "R6C1").is_some());
    assert!((r4c1.y - 10.0).abs() < 0.01);

    assert!(page_iter.next().is_none());
}