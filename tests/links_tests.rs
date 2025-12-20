mod common;

use common::fixtures::*;
use common::{TestResult, generate_pdf_from_json};
use serde_json::json;

#[test]
fn test_internal_link_to_anchor() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = block(vec![
        json!({
            "type": "Paragraph",
            "children": [{
                "type": "Hyperlink",
                "href": "#section-1",
                "children": [{ "type": "Text", "content": "Go to Section 1" }]
            }]
        }),
        page_break(),
        heading(2, "Section 1", Some("section-1")),
    ]);
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Go to Section 1");
    assert_pdf_contains_text!(pdf, "Section 1");

    let link_count = common::pdf_assertions::count_internal_links(&pdf.doc);
    assert!(
        link_count >= 1,
        "Should have at least 1 internal link, found {}",
        link_count
    );
    Ok(())
}

#[test]
fn test_external_hyperlink() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = json!({
        "type": "Paragraph",
        "children": [{
            "type": "Hyperlink",
            "href": "https://example.com",
            "children": [{ "type": "Text", "content": "Visit Example" }]
        }]
    });
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Visit Example");
    Ok(())
}

#[test]
fn test_multiple_links_same_target() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = block(vec![
        json!({
            "type": "Paragraph",
            "children": [{
                "type": "Hyperlink",
                "href": "#target",
                "children": [{ "type": "Text", "content": "Link 1" }]
            }]
        }),
        json!({
            "type": "Paragraph",
            "children": [{
                "type": "Hyperlink",
                "href": "#target",
                "children": [{ "type": "Text", "content": "Link 2" }]
            }]
        }),
        heading(2, "Target Section", Some("target")),
    ]);
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Link 1");
    assert_pdf_contains_text!(pdf, "Link 2");
    assert_pdf_contains_text!(pdf, "Target Section");

    let link_count = common::pdf_assertions::count_internal_links(&pdf.doc);
    assert!(
        link_count >= 2,
        "Should have at least 2 internal links, found {}",
        link_count
    );
    Ok(())
}

#[test]
fn test_styled_hyperlink() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "link": {
            "color": "#0000FF",
            "textDecoration": "underline"
        }
    });
    let content = json!({
        "type": "Paragraph",
        "children": [{
            "type": "Hyperlink",
            "href": "#target",
            "styleNames": ["link"],
            "children": [{ "type": "Text", "content": "Styled Link" }]
        }]
    });
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Styled Link");
    Ok(())
}

#[test]
fn test_mixed_internal_and_external_links() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = block(vec![
        json!({
            "type": "Paragraph",
            "children": [{
                "type": "Hyperlink",
                "href": "https://example.com",
                "children": [{ "type": "Text", "content": "External link" }]
            }]
        }),
        json!({
            "type": "Paragraph",
            "children": [{
                "type": "Hyperlink",
                "href": "#internal",
                "children": [{ "type": "Text", "content": "Internal link" }]
            }]
        }),
        heading(3, "Internal Target", Some("internal")),
    ]);
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "External link");
    assert_pdf_contains_text!(pdf, "Internal link");
    assert_pdf_contains_text!(pdf, "Internal Target");
    Ok(())
}
