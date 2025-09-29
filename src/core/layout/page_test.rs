use super::test_utils::{create_layout_unit, create_paragraph, create_test_engine_with_page};
use crate::core::idf::IRNode;
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::stylesheet::ElementStyle;

#[test]
fn test_simple_page_break() {
    let engine = create_test_engine_with_page(500.0, 200.0, 10.0);
    let top_margin = engine.stylesheet.page.margins.top;

    let mut children = Vec::new();
    for i in 0..20 {
        // Paragraphs have no default margin in this setup, so they stack tightly.
        children.push(create_paragraph(&format!("Line {}", i + 1)));
    }
    let tree = IRNode::Root(children);
    let layout_unit = create_layout_unit(tree);

    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();

    // Page 1
    let page1 = page_iter.next().expect("Should have a first page");
    assert!(!page1.is_empty(), "Page 1 should not be empty");
    assert_eq!(page1.first().unwrap().y, top_margin); // Starts at top margin
    // A single line is 14.4pts high. Page content height is 180. 180 / 14.4 is ~12.5.
    // So we expect 12 lines.
    assert_eq!(page1.len(), 12, "Page 1 has an unexpected number of lines");

    // Page 2
    let page2 = page_iter.next().expect("Should have a second page");
    assert!(!page2.is_empty(), "Page 2 should not be empty");
    assert_eq!(
        page2.first().unwrap().y,
        top_margin,
        "Page 2 should start at the top margin"
    );
    assert_eq!(page2.len(), 8, "Page 2 should have the remaining 8 lines");

    // No more pages
    assert!(page_iter.next().is_none(), "Should only have two pages");
}


#[test]
fn test_element_taller_than_page_is_skipped() {
    let engine = create_test_engine_with_page(500.0, 200.0, 10.0);
    let page_content_height = 180.0;

    let too_tall_block = IRNode::Block {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            height: Some(Dimension::Pt(page_content_height + 1.0)),
            background_color: Some(Default::default()), // Make it renderable
            ..Default::default()
        }),
        children: vec![create_paragraph("This block is too tall.")],
    };

    let following_paragraph = create_paragraph("This paragraph should appear on page 1.");

    let tree = IRNode::Root(vec![too_tall_block, following_paragraph]);
    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();

    let page1 = page_iter.next().expect("Should have one page");
    // The tall block is skipped. The background is one element, the paragraph's text is another.
    assert_eq!(
        page1.len(),
        1,
        "Only the second, valid paragraph should be rendered"
    );
    // Check that the rendered element is indeed the second one.
    if let super::LayoutElement::Text(text_el) = &page1[0].element {
        assert!(text_el.content.contains("should appear"));
    } else {
        panic!("Expected a text element");
    }

    assert!(page_iter.next().is_none(), "Should be no second page");
}

#[test]
fn test_nested_block_indentation() {
    let engine = create_test_engine_with_page(600.0, 800.0, 50.0);
    let page_margin = 50.0;

    let tree = IRNode::Root(vec![IRNode::Block {
        // Parent block
        style_sets: vec![],
        style_override: Some(ElementStyle {
            margin: Some(Margins { left: 20.0, ..Default::default() }),
            padding: Some(Margins { left: 10.0, ..Default::default() }),
            ..Default::default()
        }),
        children: vec![IRNode::Block {
            // Child block
            style_sets: vec![],
            style_override: Some(ElementStyle {
                margin: Some(Margins { left: 15.0, ..Default::default() }),
                padding: Some(Margins { left: 5.0, ..Default::default() }),
                ..Default::default()
            }),
            children: vec![create_paragraph("Deeply nested text.")],
        }],
    }]);

    let layout_unit = create_layout_unit(tree);
    let mut page_iter = engine.paginate_tree(layout_unit).unwrap();
    let page = page_iter.next().expect("Should produce one page");

    assert_eq!(page.len(), 1, "Should be 1 text element");
    let text_el = &page[0];

    // Calculate expected X position
    // page_margin (50) + parent_margin (20) + parent_padding (10)
    // + child_margin (15) + child_padding (5)
    let expected_x = page_margin + 20.0 + 10.0 + 15.0 + 5.0; // 100.0

    assert!(
        (text_el.x - expected_x).abs() < 0.01,
        "Text should be indented by all parent margins and paddings. Expected {}, got {}",
        expected_x,
        text_el.x
    );
}