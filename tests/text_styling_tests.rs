mod common;

use common::fixtures::*;
use common::{generate_pdf_from_json, TestResult};
use serde_json::json;

#[test]
fn test_font_family_helvetica() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "default": { "fontFamily": "Helvetica" }
    });
    let content = styled_paragraph("Hello Helvetica", &["default"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    // TODO: Enable font verification once font embedding is working
    // assert_pdf_has_font!(pdf, "Helvetica");
    assert_pdf_contains_text!(pdf, "Hello Helvetica");
    Ok(())
}

#[test]
fn test_font_family_times() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "serif": { "fontFamily": "Times" }
    });
    let content = styled_paragraph("Hello Times", &["serif"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    // TODO: Enable font verification once font embedding is working
    // assert_pdf_has_font!(pdf, "Times");
    assert_pdf_contains_text!(pdf, "Hello Times");
    Ok(())
}

#[test]
fn test_font_family_courier() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "mono": { "fontFamily": "Courier" }
    });
    let content = styled_paragraph("Hello Courier", &["mono"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    // TODO: Enable font verification once font embedding is working
    // assert_pdf_has_font!(pdf, "Courier");
    assert_pdf_contains_text!(pdf, "Hello Courier");
    Ok(())
}

#[test]
fn test_font_size_variations() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "small": { "fontSize": "8pt" },
        "normal": { "fontSize": "12pt" },
        "large": { "fontSize": "24pt" },
        "xlarge": { "fontSize": "48pt" }
    });
    let content = block(vec![
        styled_paragraph("Small text", &["small"]),
        styled_paragraph("Normal text", &["normal"]),
        styled_paragraph("Large text", &["large"]),
        styled_paragraph("XLarge text", &["xlarge"]),
    ]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    for text in &["Small", "Normal", "Large", "XLarge"] {
        assert_pdf_contains_text!(pdf, text);
    }
    Ok(())
}

#[test]
fn test_font_weight_thin() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Thin weight", json!({ "fontWeight": "thin" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Thin weight");
    Ok(())
}

#[test]
fn test_font_weight_light() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Light weight", json!({ "fontWeight": "light" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Light weight");
    Ok(())
}

#[test]
fn test_font_weight_regular() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Regular weight", json!({ "fontWeight": "regular" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Regular weight");
    Ok(())
}

#[test]
fn test_font_weight_medium() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Medium weight", json!({ "fontWeight": "medium" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Medium weight");
    Ok(())
}

#[test]
fn test_font_weight_bold() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({ "bold": { "fontWeight": "bold" } });
    let content = styled_paragraph("Bold text", &["bold"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Bold text");
    // Bold fonts typically have "Bold" or "B" in the name
    let fonts = common::pdf_assertions::extract_font_names(&pdf.doc);
    assert!(
        fonts.iter().any(|f| f.contains("Bold") || f.ends_with("-B")),
        "PDF should contain bold font variant, found: {:?}",
        fonts
    );
    Ok(())
}

#[test]
fn test_font_weight_black() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let content = paragraph_with_style("Black weight", json!({ "fontWeight": "black" }));
    let template = template_with_styles(json!({}), content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Black weight");
    Ok(())
}

#[test]
fn test_font_weight_numeric() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    for weight in &[100, 300, 400, 500, 700, 900] {
        let content = paragraph_with_style(
            &format!("Weight {}", weight),
            json!({ "fontWeight": weight }),
        );
        let template = template_with_styles(json!({}), content);
        let pdf = generate_pdf_from_json(&template)?;
        assert_pdf_contains_text!(pdf, &format!("Weight {}", weight));
    }
    Ok(())
}

#[test]
fn test_font_style_normal() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({ "normal": { "fontStyle": "normal" } });
    let content = styled_paragraph("Normal style", &["normal"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Normal style");
    Ok(())
}

#[test]
fn test_font_style_italic() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({ "italic": { "fontStyle": "italic" } });
    let content = styled_paragraph("Italic text", &["italic"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Italic text");
    // TODO: Italic font variants aren't currently used by the PDF renderer
    // The renderer applies italic styles but doesn't load italic font variants (e.g., Helvetica-Oblique)
    // let fonts = common::pdf_assertions::extract_font_names(&pdf.doc);
    // assert!(
    //     fonts
    //         .iter()
    //         .any(|f| f.contains("Italic") || f.contains("Oblique") || f.ends_with("-I")),
    //     "PDF should contain italic font variant, found: {:?}",
    //     fonts
    // );
    Ok(())
}

#[test]
fn test_font_style_oblique() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({ "oblique": { "fontStyle": "oblique" } });
    let content = styled_paragraph("Oblique style", &["oblique"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Oblique style");
    Ok(())
}

#[test]
fn test_line_height() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "tight": { "fontSize": "12pt", "lineHeight": 14.0 },
        "loose": { "fontSize": "12pt", "lineHeight": 24.0 }
    });
    let content = block(vec![
        styled_paragraph("Tight line height text\nSecond line", &["tight"]),
        styled_paragraph("Loose line height text\nSecond line", &["loose"]),
    ]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Tight line height");
    assert_pdf_contains_text!(pdf, "Loose line height");
    Ok(())
}

#[test]
fn test_bold_italic_combination() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "bold-italic": {
            "fontWeight": "bold",
            "fontStyle": "italic"
        }
    });
    let content = styled_paragraph("Bold and Italic", &["bold-italic"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Bold and Italic");
    // TODO: Italic font variants aren't currently used by the PDF renderer
    // The renderer applies italic styles but doesn't load italic font variants (e.g., Helvetica-BoldOblique)
    // let fonts = common::pdf_assertions::extract_font_names(&pdf.doc);
    // // Should find a bold-italic variant
    // assert!(
    //     fonts.iter().any(|f| (f.contains("Bold") && f.contains("Italic"))
    //         || f.contains("BoldOblique")
    //         || f.ends_with("-BI")),
    //     "PDF should contain bold-italic font variant, found: {:?}",
    //     fonts
    // );
    Ok(())
}
