mod common;

use common::fixtures::*;
use common::{generate_pdf_from_json, TestResult};
use serde_json::json;

#[test]
fn test_text_align_left() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Left aligned text here.", json!({ "textAlign": "left" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Left aligned");
    Ok(())
}

#[test]
fn test_text_align_right() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Right aligned text here.", json!({ "textAlign": "right" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Right aligned");
    Ok(())
}

#[test]
fn test_text_align_center() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Center aligned text here.", json!({ "textAlign": "center" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Center aligned");
    Ok(())
}

#[test]
fn test_text_align_justify() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let long_text = "This is a longer paragraph that should be justified. \
                     Justification spreads words across the full line width.";
    let content = paragraph_with_style(long_text, json!({ "textAlign": "justify" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "paragraph");
    Ok(())
}

#[test]
fn test_text_decoration_none() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("No decoration", json!({ "textDecoration": "none" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "No decoration");
    Ok(())
}

#[test]
fn test_text_decoration_underline() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "underline": { "textDecoration": "underline" }
    });
    let content = styled_paragraph("Underlined text", &["underline"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Underlined text");
    Ok(())
}

#[test]
fn test_text_decoration_line_through() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "strike": { "textDecoration": "line-through" }
    });
    let content = styled_paragraph("Strikethrough text", &["strike"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Strikethrough text");
    Ok(())
}

#[test]
fn test_color_hex_rgb() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "red": { "color": "#FF0000" },
        "green": { "color": "#00FF00" },
        "blue": { "color": "#0000FF" }
    });
    let content = block(vec![
        styled_paragraph("Red text", &["red"]),
        styled_paragraph("Green text", &["green"]),
        styled_paragraph("Blue text", &["blue"]),
    ]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Red text");
    assert_pdf_contains_text!(pdf, "Green text");
    assert_pdf_contains_text!(pdf, "Blue text");
    Ok(())
}

#[test]
fn test_color_hex_short() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style(
        "Short hex color",
        json!({ "color": "#F00" }), // Short form for #FF0000
    );
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Short hex");
    Ok(())
}

#[test]
fn test_color_multiple_elements() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "red": { "color": "#FF0000" },
        "blue": { "color": "#0000FF" }
    });
    let content = block(vec![
        styled_paragraph("First red paragraph", &["red"]),
        styled_paragraph("Blue paragraph", &["blue"]),
        styled_paragraph("Second red paragraph", &["red"]),
    ]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "First red");
    assert_pdf_contains_text!(pdf, "Blue paragraph");
    assert_pdf_contains_text!(pdf, "Second red");
    Ok(())
}

#[test]
fn test_combined_formatting() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "fancy": {
            "textAlign": "center",
            "textDecoration": "underline",
            "color": "#0066CC",
            "fontWeight": "bold",
            "fontSize": "18pt"
        }
    });
    let content = styled_paragraph("Fancy formatted text", &["fancy"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Fancy formatted text");
    Ok(())
}
