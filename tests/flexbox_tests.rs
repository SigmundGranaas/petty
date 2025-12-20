mod common;

use common::fixtures::*;
use common::{TestResult, generate_pdf_from_json};
use serde_json::json;

#[test]
fn test_flex_direction_row() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "flex-row": { "flexDirection": "row" },
        "item": { "width": "100pt" }
    });
    let children = vec![
        styled_flex_item("Item 1", &["item"]),
        styled_flex_item("Item 2", &["item"]),
    ];
    let content = styled_flex_container(children, &["flex-row"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Item 1");
    assert_pdf_contains_text!(pdf, "Item 2");
    Ok(())
}

#[test]
fn test_flex_direction_row_reverse() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "flex-row-reverse": { "flexDirection": "row-reverse" },
        "item": { "width": "100pt" }
    });
    let children = vec![
        styled_flex_item("First", &["item"]),
        styled_flex_item("Second", &["item"]),
    ];
    let content = styled_flex_container(children, &["flex-row-reverse"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "First");
    assert_pdf_contains_text!(pdf, "Second");
    Ok(())
}

#[test]
fn test_flex_direction_column() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "flex-column": { "flexDirection": "column" }
    });
    let children = vec![
        json!({ "type": "Block", "children": [paragraph("Column 1")] }),
        json!({ "type": "Block", "children": [paragraph("Column 2")] }),
    ];
    let content = styled_flex_container(children, &["flex-column"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Column 1");
    assert_pdf_contains_text!(pdf, "Column 2");
    Ok(())
}

#[test]
fn test_flex_direction_column_reverse() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "flex-column-reverse": { "flexDirection": "column-reverse" }
    });
    let children = vec![
        json!({ "type": "Block", "children": [paragraph("Col Rev 1")] }),
        json!({ "type": "Block", "children": [paragraph("Col Rev 2")] }),
    ];
    let content = styled_flex_container(children, &["flex-column-reverse"]);
    let template = template_with_styles(styles, content);

    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Col Rev 1");
    Ok(())
}

#[test]
fn test_flex_wrap_nowrap() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "flex-nowrap": { "flexWrap": "nowrap" },
        "item-200": { "width": "200pt" }
    });
    let children = vec![
        styled_flex_item("Wrap 1", &["item-200"]),
        styled_flex_item("Wrap 2", &["item-200"]),
        styled_flex_item("Wrap 3", &["item-200"]),
    ];
    let content = styled_flex_container(children, &["flex-nowrap"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Wrap 1");
    Ok(())
}

#[test]
fn test_flex_wrap_wrap() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "flex-wrap": { "flexWrap": "wrap" },
        "item-200": { "width": "200pt" }
    });
    let children = vec![
        styled_flex_item("W1", &["item-200"]),
        styled_flex_item("W2", &["item-200"]),
        styled_flex_item("W3", &["item-200"]),
    ];
    let content = styled_flex_container(children, &["flex-wrap"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "W1");
    assert_pdf_contains_text!(pdf, "W3");
    Ok(())
}

#[test]
fn test_justify_content_flex_start() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "justify-start": { "justifyContent": "flex-start" },
        "item": { "width": "100pt" }
    });
    let children = vec![styled_flex_item("JC Start", &["item"])];
    let content = styled_flex_container(children, &["justify-start"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "JC Start");
    Ok(())
}

#[test]
fn test_justify_content_flex_end() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "justify-end": { "justifyContent": "flex-end" },
        "item": { "width": "100pt" }
    });
    let children = vec![styled_flex_item("JC End", &["item"])];
    let content = styled_flex_container(children, &["justify-end"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "JC End");
    Ok(())
}

#[test]
fn test_justify_content_center() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "justify-center": { "justifyContent": "center" },
        "item": { "width": "100pt" }
    });
    let children = vec![styled_flex_item("JC Center", &["item"])];
    let content = styled_flex_container(children, &["justify-center"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "JC Center");
    Ok(())
}

#[test]
fn test_justify_content_space_between() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "justify-between": { "justifyContent": "space-between" },
        "item": { "width": "50pt" }
    });
    let children = vec![
        styled_flex_item("A", &["item"]),
        styled_flex_item("B", &["item"]),
    ];
    let content = styled_flex_container(children, &["justify-between"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "A");
    assert_pdf_contains_text!(pdf, "B");
    Ok(())
}

#[test]
fn test_justify_content_space_around() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "justify-around": { "justifyContent": "space-around" },
        "item": { "width": "50pt" }
    });
    let children = vec![
        styled_flex_item("X", &["item"]),
        styled_flex_item("Y", &["item"]),
    ];
    let content = styled_flex_container(children, &["justify-around"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "X");
    Ok(())
}

#[test]
fn test_align_items_stretch() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "align-stretch": {
            "alignItems": "stretch",
            "height": "100pt"
        }
    });
    let children = vec![json!({ "type": "Block", "children": [paragraph("Stretch")] })];
    let content = styled_flex_container(children, &["align-stretch"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Stretch");
    Ok(())
}

#[test]
fn test_align_items_flex_start() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "align-start": {
            "alignItems": "flex-start",
            "height": "100pt"
        }
    });
    let children = vec![json!({ "type": "Block", "children": [paragraph("AI Start")] })];
    let content = styled_flex_container(children, &["align-start"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "AI Start");
    Ok(())
}

#[test]
fn test_align_items_flex_end() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "align-end": {
            "alignItems": "flex-end",
            "height": "100pt"
        }
    });
    let children = vec![json!({ "type": "Block", "children": [paragraph("AI End")] })];
    let content = styled_flex_container(children, &["align-end"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "AI End");
    Ok(())
}

#[test]
fn test_align_items_center() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "align-center": {
            "alignItems": "center",
            "height": "100pt"
        }
    });
    let children = vec![json!({ "type": "Block", "children": [paragraph("AI Center")] })];
    let content = styled_flex_container(children, &["align-center"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "AI Center");
    Ok(())
}

#[test]
fn test_flex_order() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "order-2": { "order": 2, "width": "100pt" },
        "order-1": { "order": 1, "width": "100pt" }
    });
    let children = vec![
        styled_flex_item("Order 2", &["order-2"]),
        styled_flex_item("Order 1", &["order-1"]),
    ];
    let content = json!({
        "type": "FlexContainer",
        "children": children
    });
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Order 1");
    assert_pdf_contains_text!(pdf, "Order 2");
    Ok(())
}

#[test]
fn test_flex_grow() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "grow-1": { "flexGrow": 1 },
        "grow-2": { "flexGrow": 2 }
    });
    let children = vec![
        styled_flex_item("Grow 1", &["grow-1"]),
        styled_flex_item("Grow 2", &["grow-2"]),
    ];
    let content = json!({
        "type": "FlexContainer",
        "children": children
    });
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Grow 1");
    assert_pdf_contains_text!(pdf, "Grow 2");
    Ok(())
}

#[test]
fn test_flex_shrink() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "shrink-1": { "flexBasis": "300pt", "flexShrink": 1 },
        "shrink-2": { "flexBasis": "300pt", "flexShrink": 2 }
    });
    let children = vec![
        styled_flex_item("Shrink 1", &["shrink-1"]),
        styled_flex_item("Shrink 2", &["shrink-2"]),
    ];
    let content = json!({
        "type": "FlexContainer",
        "children": children
    });
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Shrink 1");
    Ok(())
}

#[test]
fn test_flex_basis() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "basis-100": { "flexBasis": "100pt" },
        "basis-200": { "flexBasis": "200pt" }
    });
    let children = vec![
        styled_flex_item("Basis 100", &["basis-100"]),
        styled_flex_item("Basis 200", &["basis-200"]),
    ];
    let content = json!({
        "type": "FlexContainer",
        "children": children
    });
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Basis 100");
    Ok(())
}

#[test]
fn test_align_self() -> TestResult {
    let _ = env_logger::builder().is_test(true).try_init();

    let styles = json!({
        "container": { "height": "100pt" },
        "self-start": { "alignSelf": "flex-start", "height": "30pt" },
        "self-end": { "alignSelf": "flex-end", "height": "30pt" }
    });
    let children = vec![
        styled_flex_item("Self Start", &["self-start"]),
        styled_flex_item("Self End", &["self-end"]),
    ];
    let content = styled_flex_container(children, &["container"]);
    let template = template_with_styles(styles, content);
    let pdf = generate_pdf_from_json(&template)?;
    assert_pdf_contains_text!(pdf, "Self Start");
    assert_pdf_contains_text!(pdf, "Self End");
    Ok(())
}
