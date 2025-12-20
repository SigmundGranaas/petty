mod common;

use common::fixtures::*;
use common::{TestResult, generate_pdf_from_json};
use serde_json::json;

#[test]
fn test_background_color() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "highlighted": { "backgroundColor": "#FFFF00" }
    });
    let content = styled_paragraph("Highlighted text", &["highlighted"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Highlighted text");
    Ok(())
}

#[test]
fn test_border_all_sides() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "bordered": { "border": "2pt solid #000000" }
    });
    let content = styled_paragraph("Bordered text", &["bordered"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Bordered text");
    Ok(())
}

#[test]
fn test_border_individual_sides() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "custom-border": {
            "borderTop": "1pt solid #FF0000",
            "borderRight": "2pt dashed #00FF00",
            "borderBottom": "3pt dotted #0000FF",
            "borderLeft": "4pt double #000000"
        }
    });
    let content = styled_paragraph("Individual borders", &["custom-border"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Individual borders");
    Ok(())
}

#[test]
fn test_border_style_solid() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "bordered": { "border": "2pt solid #000000" }
    });
    let content = styled_paragraph("Solid border", &["bordered"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Solid border");
    Ok(())
}

#[test]
fn test_border_style_dashed() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "bordered": { "border": "2pt dashed #000000" }
    });
    let content = styled_paragraph("Dashed border", &["bordered"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Dashed border");
    Ok(())
}

#[test]
fn test_border_style_dotted() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "bordered": { "border": "2pt dotted #000000" }
    });
    let content = styled_paragraph("Dotted border", &["bordered"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Dotted border");
    Ok(())
}

#[test]
fn test_border_style_double() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "bordered": { "border": "4pt double #000000" }
    });
    let content = styled_paragraph("Double border", &["bordered"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Double border");
    Ok(())
}

#[test]
fn test_margin_single_value() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Margin all sides", json!({ "margin": "20pt" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Margin all sides");
    Ok(())
}

#[test]
fn test_margin_individual() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "spaced": {
            "marginTop": "10pt",
            "marginRight": "20pt",
            "marginBottom": "30pt",
            "marginLeft": "40pt"
        }
    });
    let content = styled_paragraph("Individual margins", &["spaced"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Individual margins");
    Ok(())
}

#[test]
fn test_padding_single_value() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style(
        "Padding all sides",
        json!({ "padding": "15pt", "backgroundColor": "#EEEEEE" }),
    );
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Padding all sides");
    Ok(())
}

#[test]
fn test_padding_individual() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "padded": {
            "paddingTop": "5pt",
            "paddingRight": "10pt",
            "paddingBottom": "15pt",
            "paddingLeft": "20pt",
            "backgroundColor": "#EEEEEE"
        }
    });
    let content = styled_paragraph("Individual padding", &["padded"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Individual padding");
    Ok(())
}

#[test]
fn test_width_fixed() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "fixed-width": { "width": "200pt", "border": "1pt solid #000" }
    });
    let p = paragraph("Fixed width block");
    let content = json!({
        "type": "Block",
        "styleNames": ["fixed-width"],
        "children": [p]
    });
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Fixed width");
    Ok(())
}

#[test]
fn test_width_percent() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "percent-width": { "width": { "percent": 50.0 }, "border": "1pt solid #000" }
    });
    let p = paragraph("50% width block");
    let content = json!({
        "type": "Block",
        "styleNames": ["percent-width"],
        "children": [p]
    });
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "50% width");
    Ok(())
}

#[test]
fn test_height_fixed() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "fixed-height": { "height": "100pt", "border": "1pt solid #000" }
    });
    let p = paragraph("Fixed height block");
    let content = json!({
        "type": "Block",
        "styleNames": ["fixed-height"],
        "children": [p]
    });
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Fixed height");
    Ok(())
}

#[test]
fn test_combined_box_model() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "boxed": {
            "backgroundColor": "#F0F0F0",
            "border": "2pt solid #333333",
            "margin": "10pt",
            "padding": "15pt",
            "width": "300pt"
        }
    });
    let content = styled_paragraph("Complete box model", &["boxed"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Complete box model");
    Ok(())
}
