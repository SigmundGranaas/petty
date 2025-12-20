mod common;

use common::fixtures::*;
use common::{generate_pdf_from_json, TestResult};
use serde_json::json;

#[test]
fn test_page_size_a4() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = template_with_page_settings("A4", "0pt");
    let pdf = generate_pdf_from_json(&template)?;

    // A4 dimensions: 595.28 x 841.89 points
    assert_pdf_page_size!(pdf, 1, 595.28, 841.89);
    Ok(())
}

#[test]
fn test_page_size_letter() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = template_with_page_settings("Letter", "0pt");
    let pdf = generate_pdf_from_json(&template)?;

    // Letter: 612 x 792 points
    assert_pdf_page_size!(pdf, 1, 612.0, 792.0);
    Ok(())
}

#[test]
fn test_page_size_legal() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = template_with_page_settings("Legal", "0pt");
    let pdf = generate_pdf_from_json(&template)?;

    // Legal: 612 x 1008 points
    assert_pdf_page_size!(pdf, 1, 612.0, 1008.0);
    Ok(())
}

#[test]
fn test_custom_page_size() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": { "width": 400.0, "height": 600.0 },
                    "margins": "0pt"
                }
            },
            "styles": {}
        },
        "_template": { "type": "Block", "children": [] }
    });

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_size!(pdf, 1, 400.0, 600.0);
    Ok(())
}

#[test]
fn test_margins_single_value() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    // Margin "2cm" applies to all sides
    let template = template_with_page_settings("A4", "2cm");
    let pdf = generate_pdf_from_json(&template)?;

    // Just verify the PDF is generated
    assert_pdf_page_count!(pdf, 1);
    Ok(())
}

#[test]
fn test_margins_multiple_values() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": "A4",
                    "margins": "10pt 20pt 30pt 40pt"  // top right bottom left
                }
            },
            "styles": {}
        },
        "_template": block(vec![paragraph("Test content")])
    });

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Test content");
    Ok(())
}

#[test]
fn test_margin_units_pt() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = template_with_page_settings("A4", "72pt");
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 1);
    Ok(())
}

#[test]
fn test_margin_units_in() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = template_with_page_settings("A4", "1in");
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 1);
    Ok(())
}

#[test]
fn test_margin_units_cm() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = template_with_page_settings("A4", "2.54cm");
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 1);
    Ok(())
}

#[test]
fn test_margin_units_mm() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let template = template_with_page_settings("A4", "25.4mm");
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 1);
    Ok(())
}
