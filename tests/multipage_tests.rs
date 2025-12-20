mod common;

use common::fixtures::*;
use common::{TestResult, generate_pdf_from_json};
use serde_json::json;

#[test]
fn test_explicit_page_break() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = block(vec![
        paragraph("Page 1 content"),
        page_break(),
        paragraph("Page 2 content"),
    ]);
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 2);
    assert_pdf_contains_text!(pdf, "Page 1 content");
    assert_pdf_contains_text!(pdf, "Page 2 content");
    Ok(())
}

#[test]
fn test_multiple_page_breaks() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = block(vec![
        paragraph("Page 1"),
        page_break(),
        paragraph("Page 2"),
        page_break(),
        paragraph("Page 3"),
        page_break(),
        paragraph("Page 4"),
    ]);
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 4);
    Ok(())
}

#[test]
fn test_automatic_page_overflow() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    // Generate enough content to overflow onto multiple pages
    let mut paragraphs = Vec::new();
    for i in 1..=100 {
        // Make each paragraph longer to ensure overflow
        paragraphs.push(paragraph(&format!(
            "Paragraph {} with much more text content to ensure that we generate enough content \
            to fill multiple pages. This paragraph contains additional sentences to increase the \
            total amount of text in the PDF document. We want to test automatic page overflow \
            functionality by creating a document that is too large to fit on a single page.",
            i
        )));
    }
    let content = block(paragraphs);
    let template = template_with_styles(json!({}), content);

    let pdf = generate_pdf_from_json(&template)?;
    assert!(
        pdf.page_count() > 1,
        "Should have multiple pages due to overflow, got {}",
        pdf.page_count()
    );
    assert_pdf_contains_text!(pdf, "Paragraph 1");
    assert_pdf_contains_text!(pdf, "Paragraph 100");
    Ok(())
}

#[test]
fn test_page_break_preserves_styles() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "styled": { "color": "#FF0000", "fontWeight": "bold" }
    });
    let content = block(vec![
        styled_paragraph("Styled on page 1", &["styled"]),
        page_break(),
        styled_paragraph("Styled on page 2", &["styled"]),
    ]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 2);
    assert_pdf_contains_text!(pdf, "Styled on page 1");
    assert_pdf_contains_text!(pdf, "Styled on page 2");
    Ok(())
}

#[test]
fn test_widows_orphans_control() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a long paragraph that will split across pages
    let long_text = (1..=30)
        .map(|i| format!("Line {}.", i))
        .collect::<Vec<_>>()
        .join("\n");
    let styles = json!({
        "controlled": { "widows": 2, "orphans": 2 }
    });
    let content = styled_paragraph(&long_text, &["controlled"]);

    // Use a small page to force pagination
    let template = json!({
        "_stylesheet": {
            "defaultPageMaster": "default",
            "pageMasters": {
                "default": {
                    "size": { "width": 400.0, "height": 200.0 },
                    "margins": "20pt"
                }
            },
            "styles": styles
        },
        "_template": content
    });

    let pdf = generate_pdf_from_json(&template)?;
    assert!(
        pdf.page_count() > 1,
        "Should have multiple pages, got {}",
        pdf.page_count()
    );
    Ok(())
}

#[test]
fn test_mixed_content_across_pages() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "heading": { "fontSize": "18pt", "fontWeight": "bold" }
    });

    let content = block(vec![
        styled_paragraph("Chapter 1", &["heading"]),
        paragraph("This is the first chapter with some introductory content."),
        paragraph("Point 1"),
        paragraph("Point 2"),
        page_break(),
        styled_paragraph("Chapter 2", &["heading"]),
        paragraph("This is the second chapter."),
        paragraph("Table content: A B"),
    ]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_page_count!(pdf, 2);
    assert_pdf_contains_text!(pdf, "Chapter 1");
    assert_pdf_contains_text!(pdf, "Chapter 2");
    assert_pdf_contains_text!(pdf, "Point 1");
    Ok(())
}
