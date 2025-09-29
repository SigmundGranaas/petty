#![cfg(test)]

use crate::core::idf::{TableColumnDefinition};
use crate::core::style::dimension::Dimension;
use crate::core::layout::table::calculate_column_widths;

#[test]
fn test_calculate_column_widths_mixed() {
    let table_width = 1000.0;
    let columns = vec![
        TableColumnDefinition { // Fixed
            width: Some(Dimension::Pt(200.0)),
            ..Default::default()
        },
        TableColumnDefinition { // Percent
            width: Some(Dimension::Percent(30.0)),
            ..Default::default()
        },
        TableColumnDefinition { // Auto
            width: Some(Dimension::Auto),
            ..Default::default()
        },
        TableColumnDefinition { // Auto (None)
            width: None,
            ..Default::default()
        },
    ];

    let widths = calculate_column_widths(&columns, table_width);

    // Fixed width
    assert_eq!(widths[0], 200.0);
    // Percent width
    assert_eq!(widths[1], 300.0);
    // Remaining width = 1000 - 200 - 300 = 500
    // Split between 2 auto columns = 250 each
    assert_eq!(widths[2], 250.0);
    assert_eq!(widths[3], 250.0);
}

#[test]
fn test_calculate_column_widths_all_auto() {
    let table_width = 800.0;
    let columns = vec![
        TableColumnDefinition { width: None, ..Default::default() },
        TableColumnDefinition { width: None, ..Default::default() },
        TableColumnDefinition { width: None, ..Default::default() },
        TableColumnDefinition { width: None, ..Default::default() },
    ];
    let widths = calculate_column_widths(&columns, table_width);
    assert_eq!(widths.len(), 4);
    for width in widths {
        assert_eq!(width, 200.0);
    }
}