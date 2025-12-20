#![cfg(test)]

use crate::test_utils::{create_paragraph, find_first_text_box_with_content, paginate_test_nodes};
use petty_idf::{IRNode, InlineNode, NodeMetadata};
use petty_style::dimension::{Margins, PageSize};
use petty_style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use petty_style::text::TextAlign;
use std::collections::HashMap;

#[test]
fn test_text_wrapping() {
    // Page width 220, margin 10 -> content width 200
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 220.0,
                    height: 500.0,
                },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let text = "This is a very very long line of text that is absolutely guaranteed to wrap at least once.";
    let nodes = vec![create_paragraph(text)];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    // The text will be broken into multiple PositionedElements (runs).
    // We check if any of them are on a y-coordinate corresponding to the second line.
    let default_line_height = 14.4; // From default style.
    let second_line_y = 10.0 + default_line_height; // page_margin_top + one line_height

    let has_second_line = page1.iter().any(|el| (el.y - second_line_y).abs() < 0.1);

    assert!(
        has_second_line,
        "Paragraph should have wrapped to a second line."
    );
}

#[test]
fn test_text_alignment_center() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: 500.0,
                },
                margins: Some(Margins::all(10.0)), // content width 480
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let content_width = 480.0;

    let text = "Centered text";
    let style_override = ElementStyle {
        text_align: Some(TextAlign::Center),
        ..Default::default()
    };

    let para = IRNode::Paragraph {
        meta: NodeMetadata {
            id: None,
            style_sets: vec![],
            style_override: Some(style_override),
        },
        children: vec![InlineNode::Text(text.to_string())],
    };

    let nodes = vec![para];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    let text_el = find_first_text_box_with_content(page1, text).unwrap();

    let engine = crate::test_utils::create_test_engine();
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

#[test]
fn test_text_justification() {
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                // Use a smaller width to force a line wrap where the test expects it.
                // With width=150, content_width=130.
                // width("First word on this ") is ~85pt. "+ justified" is ~50pt. Total ~135pt > 130.
                // So the line wrap will be before "justified", making "this" the last word on the line.
                size: PageSize::Custom {
                    width: 150.0,
                    height: 500.0,
                },
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };
    let content_width = 130.0;
    let text = "First word on this justified line.";
    let style_override = ElementStyle {
        text_align: Some(TextAlign::Justify),
        ..Default::default()
    };
    let para = IRNode::Paragraph {
        meta: NodeMetadata {
            style_override: Some(style_override),
            ..Default::default()
        },
        children: vec![InlineNode::Text(text.to_string())],
    };
    let nodes = vec![para];
    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();
    let page1 = &pages[0];

    let word1 = find_first_text_box_with_content(page1, "First").unwrap();
    let word2 = find_first_text_box_with_content(page1, "word").unwrap();
    let word4 = find_first_text_box_with_content(page1, "this").unwrap();

    // Check that all words are on the first line
    assert!((word1.y - word4.y).abs() < 0.1);

    // First word at left margin
    assert!((word1.x - 10.0).abs() < 0.1);

    // End of last word at right margin
    let end_x = word4.x + word4.width;
    assert!(
        (end_x - (10.0 + content_width)).abs() < 1.0,
        "Last word should end at right margin. Got {}",
        end_x
    );

    // Spaces are stretched
    let engine = crate::test_utils::create_test_engine();
    let style = engine.get_default_style();
    let normal_space_width = engine.measure_text_width(" ", &style);
    let space1_width = word2.x - (word1.x + word1.width);
    assert!(
        space1_width > normal_space_width,
        "Space should be stretched."
    );

    // Last line is not justified
    let last_line_word = find_first_text_box_with_content(page1, "justified").unwrap();
    assert!(
        (last_line_word.x - 10.0).abs() < 0.1,
        "Last line should be left-aligned."
    );
}

#[test]
fn test_widow_control() {
    // Page content height 50. Line height 14.4. Can fit 3 lines (43.2 used).
    // A 4-line paragraph would normally break after line 3, leaving 1 line (a widow) on page 2.
    // With widows: 2, it should instead break after line 2, moving 2 lines to page 2.
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: 70.0,
                },
                margins: Some(Margins::all(10.0)), // content height = 50
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    // Use the helper which correctly creates LineBreak nodes
    let mut para = create_paragraph("Line 1\nLine 2\nLine 3\nLine 4");
    if let IRNode::Paragraph {
        meta: NodeMetadata {
            ref mut style_override,
            ..
        },
        ..
    } = para
    {
        *style_override = Some(ElementStyle {
            widows: Some(2),
            ..Default::default()
        });
    } else {
        panic!("Expected a paragraph node");
    }

    let (pages, _, _) = paginate_test_nodes(stylesheet, vec![para]).unwrap();

    assert_eq!(pages.len(), 2, "Expected paragraph to split into two pages");

    // Page 1 should only have 2 lines due to widow control.
    let page1 = &pages[0];
    assert!(find_first_text_box_with_content(page1, "Line 1").is_some());
    assert!(find_first_text_box_with_content(page1, "Line 2").is_some());
    assert!(find_first_text_box_with_content(page1, "Line 3").is_none());
    // Checking element count is tricky due to runs. Content check is better.

    // Page 2 should have the remaining 2 lines.
    let page2 = &pages[1];
    assert!(find_first_text_box_with_content(page2, "Line 3").is_some());
    assert!(find_first_text_box_with_content(page2, "Line 4").is_some());
}

#[test]
fn test_orphan_control() {
    // Page content height 50. Line height 14.4. Can fit 3 lines.
    // We have a 2-line para, then a 3-line para.
    // After the 2-line para (2 * 14.4 = 28.8), 50 - 28.8 = 21.2 space left.
    // Only 1 line of the second para fits.
    // With orphans: 2, this is not allowed (1 < 2). The second para should be pushed entirely to page 2.
    // The second page (height 50) is large enough to hold the 3-line paragraph (3 * 14.4 = 43.2).
    let stylesheet = Stylesheet {
        page_masters: HashMap::from([(
            "master".to_string(),
            PageLayout {
                size: PageSize::Custom {
                    width: 500.0,
                    height: 70.0,
                }, // content height = 50
                margins: Some(Margins::all(10.0)),
                ..Default::default()
            },
        )]),
        default_page_master_name: Some("master".to_string()),
        ..Default::default()
    };

    let mut para2 = create_paragraph("Orphan 1\nOrphan 2\nOrphan 3");
    if let IRNode::Paragraph {
        meta: NodeMetadata {
            ref mut style_override,
            ..
        },
        ..
    } = para2
    {
        *style_override = Some(ElementStyle {
            orphans: Some(2),
            widows: Some(1),
            ..Default::default()
        });
    } else {
        panic!("Expected a paragraph node");
    }

    let nodes = vec![
        create_paragraph("Before 1\nBefore 2"), // 2 lines
        para2,
    ];

    let (pages, _, _) = paginate_test_nodes(stylesheet, nodes).unwrap();

    assert_eq!(pages.len(), 2, "Expected content to split into two pages");

    let page1 = &pages[0];
    assert!(find_first_text_box_with_content(page1, "Before 2").is_some());
    assert!(
        find_first_text_box_with_content(page1, "Orphan 1").is_none(),
        "Orphan control should have pushed the second paragraph"
    );

    let page2 = &pages[1];
    assert!(find_first_text_box_with_content(page2, "Orphan 1").is_some());
    assert!(find_first_text_box_with_content(page2, "Orphan 3").is_some());
    let orphan1 = find_first_text_box_with_content(page2, "Orphan 1").unwrap();
    assert_eq!(
        orphan1.y, 10.0,
        "Second paragraph should start at the top of page 2"
    );
}
