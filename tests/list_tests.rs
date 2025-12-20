mod common;

use common::fixtures::*;
use common::{generate_pdf_from_json, TestResult};
use serde_json::json;

#[test]
fn test_list_style_disc() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![paragraph("Item one"), paragraph("Item two")];
    let content = list(items, Some("disc"));
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Item one");
    assert_pdf_contains_text!(pdf, "Item two");
    Ok(())
}

#[test]
fn test_list_style_circle() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![paragraph("Circle one"), paragraph("Circle two")];
    let content = list(items, Some("circle"));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Circle one");
    Ok(())
}

#[test]
fn test_list_style_square() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![paragraph("Square one"), paragraph("Square two")];
    let content = list(items, Some("square"));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Square one");
    Ok(())
}

#[test]
fn test_list_style_decimal() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![
        paragraph("First item"),
        paragraph("Second item"),
        paragraph("Third item"),
    ];
    let content = list(items, Some("decimal"));
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    let text = common::pdf_assertions::extract_text(&pdf.doc);
    // Should contain numeric markers (exact format may vary)
    assert!(
        text.contains("1") && text.contains("2") && text.contains("3"),
        "Should contain decimal markers 1, 2, 3. Got: {}",
        text
    );
    assert_pdf_contains_text!(pdf, "First item");
    assert_pdf_contains_text!(pdf, "Second item");
    assert_pdf_contains_text!(pdf, "Third item");
    Ok(())
}

#[test]
fn test_list_style_lower_alpha() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![paragraph("Alpha one"), paragraph("Alpha two")];
    let content = list(items, Some("lower-alpha"));
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Alpha one");
    assert_pdf_contains_text!(pdf, "Alpha two");
    Ok(())
}

#[test]
fn test_list_style_upper_alpha() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![paragraph("Upper one"), paragraph("Upper two")];
    let content = list(items, Some("upper-alpha"));
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Upper one");
    assert_pdf_contains_text!(pdf, "Upper two");
    Ok(())
}

#[test]
fn test_list_style_lower_roman() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![
        paragraph("Roman one"),
        paragraph("Roman two"),
        paragraph("Roman three"),
    ];
    let content = list(items, Some("lower-roman"));
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Roman one");
    assert_pdf_contains_text!(pdf, "Roman two");
    assert_pdf_contains_text!(pdf, "Roman three");
    Ok(())
}

#[test]
fn test_list_style_upper_roman() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![paragraph("Upper Roman one"), paragraph("Upper Roman two")];
    let content = list(items, Some("upper-roman"));
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Upper Roman one");
    assert_pdf_contains_text!(pdf, "Upper Roman two");
    Ok(())
}

#[test]
fn test_list_style_none() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = vec![paragraph("No marker")];
    let content = list(items, Some("none"));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "No marker");
    Ok(())
}

#[test]
fn test_list_position_outside() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = json!({
        "type": "List",
        "styleOverride": { "listStylePosition": "outside" },
        "children": [
            { "type": "ListItem", "children": [paragraph("Outside position")] }
        ]
    });
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Outside position");
    Ok(())
}

#[test]
fn test_list_position_inside() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = json!({
        "type": "List",
        "styleOverride": { "listStylePosition": "inside" },
        "children": [
            { "type": "ListItem", "children": [paragraph("Inside position")] }
        ]
    });
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Inside position");
    Ok(())
}

#[test]
fn test_nested_lists() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let inner_list = list(vec![paragraph("Nested item")], Some("disc"));
    let content = json!({
        "type": "List",
        "styleOverride": { "listStyleType": "decimal" },
        "children": [
            {
                "type": "ListItem",
                "children": [paragraph("Parent item"), inner_list]
            }
        ]
    });
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Parent item");
    assert_pdf_contains_text!(pdf, "Nested item");
    Ok(())
}

#[test]
fn test_list_start_number() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = json!({
        "type": "List",
        "start": 5,
        "styleOverride": { "listStyleType": "decimal" },
        "children": [
            { "type": "ListItem", "children": [paragraph("Fifth")] },
            { "type": "ListItem", "children": [paragraph("Sixth")] }
        ]
    });
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Fifth");
    assert_pdf_contains_text!(pdf, "Sixth");
    Ok(())
}

#[test]
fn test_list_multiple_items() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let items = (1..=10)
        .map(|i| paragraph(&format!("Item {}", i)))
        .collect();
    let content = list(items, Some("decimal"));
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Item 1");
    assert_pdf_contains_text!(pdf, "Item 5");
    assert_pdf_contains_text!(pdf, "Item 10");
    Ok(())
}
