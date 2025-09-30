// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/text_test.rs
#![cfg(test)]

use crate::core::idf::{IRNode, InlineNode};
use crate::core::layout::test_utils::{
    create_layout_unit, create_paragraph, create_test_engine_with_page,
    find_first_text_box_with_content,
};
use crate::core::style::stylesheet::ElementStyle;
use crate::core::style::text::TextAlign;

#[test]
fn test_text_wrapping() {
    // Page width 220, margin 10 -> content width 200
    let engine = create_test_engine_with_page(220.0, 500.0, 10.0);

    let text = "This is a very very long line of text that is absolutely guaranteed to wrap at least once.";
    let tree = IRNode::Root(vec![create_paragraph(text)]);

    let layout_unit = create_layout_unit(tree);
    let pages = engine.paginate_tree(layout_unit).unwrap();
    let page1 = &pages[0];

    // The text will be broken into multiple PositionedElements (runs).
    // We check if any of them are on a y-coordinate corresponding to the second line.
    let default_line_height = 14.4; // From default style.
    let second_line_y = 10.0 + default_line_height; // page_margin_top + one line_height

    let has_second_line = page1.iter().any(|el| (el.y - second_line_y).abs() < 0.1);

    assert!(has_second_line, "Paragraph should have wrapped to a second line.");
}

#[test]
fn test_text_alignment_center() {
    let engine = create_test_engine_with_page(500.0, 500.0, 10.0);
    let content_width = 480.0;

    let text = "Centered text";
    let style_override = ElementStyle {
        text_align: Some(TextAlign::Center),
        ..Default::default()
    };

    let para = IRNode::Paragraph {
        style_sets: vec![],
        style_override: Some(style_override),
        children: vec![InlineNode::Text(text.to_string())],
    };

    let tree = IRNode::Root(vec![para]);
    let layout_unit = create_layout_unit(tree);
    let pages = engine.paginate_tree(layout_unit).unwrap();
    let page1 = &pages[0];

    let text_el = find_first_text_box_with_content(page1, text).unwrap();

    // The text itself has a width. The box containing it (PositionedElement) has that width.
    // Its x position should be offset.
    let style = engine.get_default_style();
    let text_width = engine.measure_text_width(text, &style);

    // Expected x = page_margin + (content_width - text_width) / 2
    let expected_x = 10.0 + (content_width - text_width) / 2.0;
    assert!(
        (text_el.x - expected_x).abs() < 0.1,
        "Text not centered. Expected x={}, got {}",
        expected_x,
        text_el.x
    );
}