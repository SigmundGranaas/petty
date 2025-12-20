mod common;

use common::fixtures::*;
use common::{TestResult, generate_pdf_from_json};
use serde_json::json;

#[test]
fn test_basic_table() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = table(
        vec![
            json!({ "width": { "percent": 50.0 } }),
            json!({ "width": { "percent": 50.0 } }),
        ],
        None,
        vec![
            table_row(vec![table_cell("A"), table_cell("B")]),
            table_row(vec![table_cell("C"), table_cell("D")]),
        ],
    );
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    for cell in &["A", "B", "C", "D"] {
        assert_pdf_contains_text!(pdf, cell);
    }
    Ok(())
}

#[test]
fn test_table_with_header() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = table(
        vec![
            json!({ "width": { "percent": 33.0 } }),
            json!({ "width": { "percent": 33.0 } }),
            json!({ "width": { "percent": 34.0 } }),
        ],
        Some(vec![table_row(vec![
            table_cell("Header 1"),
            table_cell("Header 2"),
            table_cell("Header 3"),
        ])]),
        vec![table_row(vec![
            table_cell("Row 1 Col 1"),
            table_cell("Row 1 Col 2"),
            table_cell("Row 1 Col 3"),
        ])],
    );
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Header 1");
    assert_pdf_contains_text!(pdf, "Row 1 Col 1");
    Ok(())
}

#[test]
fn test_table_column_widths_percent() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = table(
        vec![
            json!({ "width": { "percent": 25.0 } }),
            json!({ "width": { "percent": 50.0 } }),
            json!({ "width": { "percent": 25.0 } }),
        ],
        None,
        vec![table_row(vec![
            table_cell("25%"),
            table_cell("50%"),
            table_cell("25%"),
        ])],
    );
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "50%");
    Ok(())
}

#[test]
fn test_table_column_widths_fixed() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = table(
        vec![
            json!({ "width": { "pt": 100.0 } }),
            json!({ "width": { "pt": 200.0 } }),
        ],
        None,
        vec![table_row(vec![
            table_cell("100pt wide"),
            table_cell("200pt wide"),
        ])],
    );
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "100pt wide");
    Ok(())
}

#[test]
fn test_table_cell_borders() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "bordered": { "border": "1pt solid #000000" }
    });
    let content = table(
        vec![json!({ "width": { "percent": 100.0 } })],
        None,
        vec![table_row(vec![styled_cell("Bordered cell", &["bordered"])])],
    );
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Bordered cell");
    Ok(())
}

#[test]
fn test_table_cell_padding() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "padded": { "padding": "10pt" }
    });
    let content = table(
        vec![json!({ "width": { "percent": 100.0 } })],
        None,
        vec![table_row(vec![styled_cell("Padded cell", &["padded"])])],
    );
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Padded cell");
    Ok(())
}

#[test]
fn test_table_cell_background() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "highlighted": { "backgroundColor": "#FFFF00" }
    });
    let content = table(
        vec![json!({ "width": { "percent": 100.0 } })],
        None,
        vec![table_row(vec![styled_cell(
            "Highlighted cell",
            &["highlighted"],
        )])],
    );
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Highlighted cell");
    Ok(())
}

#[test]
fn test_table_colspan() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    // Note: colspan/rowspan support requires additional cell metadata
    // For now, test that cells render correctly
    let content = table(
        vec![
            json!({ "width": { "percent": 33.0 } }),
            json!({ "width": { "percent": 33.0 } }),
            json!({ "width": { "percent": 34.0 } }),
        ],
        None,
        vec![table_row(vec![
            table_cell("Spans columns"),
            table_cell("Single"),
        ])],
    );
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Spans columns");
    assert_pdf_contains_text!(pdf, "Single");
    Ok(())
}

#[test]
fn test_table_rowspan() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    // Note: rowspan support requires additional cell metadata
    // For now, test that cells render correctly
    let content = table(
        vec![
            json!({ "width": { "percent": 50.0 } }),
            json!({ "width": { "percent": 50.0 } }),
        ],
        None,
        vec![
            table_row(vec![table_cell("Spans rows"), table_cell("Row 1")]),
            table_row(vec![table_cell("Row 2")]),
        ],
    );
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Spans rows");
    assert_pdf_contains_text!(pdf, "Row 1");
    assert_pdf_contains_text!(pdf, "Row 2");
    Ok(())
}

#[test]
fn test_table_multiple_rows() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let rows: Vec<_> = (1..=5)
        .map(|i| {
            table_row(vec![
                table_cell(&format!("Row {}", i)),
                table_cell(&format!("Data {}", i)),
            ])
        })
        .collect();

    let content = table(
        vec![
            json!({ "width": { "percent": 50.0 } }),
            json!({ "width": { "percent": 50.0 } }),
        ],
        None,
        rows,
    );
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Row 1");
    assert_pdf_contains_text!(pdf, "Row 5");
    Ok(())
}
