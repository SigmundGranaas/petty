#![cfg(test)]

use crate::core::idf::{IRNode, InlineNode};
use crate::core::style::text::TextAlign;
use crate::core::layout::test_utils::{create_test_engine, get_base_style};
use crate::core::layout::text::{atomize_inlines, layout_paragraph};
use crate::core::layout::LayoutContent;
use std::sync::Arc;

#[test]
fn test_text_wrapping() {
    let engine = create_test_engine();
    let style = get_base_style(); // font-size 10, line-height 12

    let text = "This is a very very long line of text that is absolutely guaranteed to wrap at least once.";
    let mut tree = IRNode::Paragraph {
        style_sets: vec![],
        style_override: None,
        children: vec![InlineNode::Text(text.to_string())],
    };

    let children = match &tree {
        IRNode::Paragraph { children, .. } => children,
        _ => panic!(),
    };
    let atoms = atomize_inlines(&engine, children, &style, None);
    let total_width: f32 = atoms.iter().map(|a| a.width()).sum();

    // Set available width to force at least one wrap.
    let available_width = total_width * 0.7;

    let layout_box = layout_paragraph(&engine, &mut tree, style, (available_width, f32::INFINITY));

    assert!(
        layout_box.rect.height > 12.0,
        "Paragraph should have wrapped to more than one line. Actual height: {}",
        layout_box.rect.height
    );

    if let LayoutContent::Children(lines) = layout_box.content {
        assert!(!lines.is_empty(), "Layout should produce child boxes for text runs");
        // Find a box that starts on the second line.
        let second_line_box = lines.iter().find(|b| (b.rect.y - 12.0).abs() < 0.1);
        assert!(second_line_box.is_some(), "Could not find any content on the second line");
    } else {
        panic!("Paragraph layout did not produce children");
    }
}

#[test]
fn test_text_alignment_center() {
    let engine = create_test_engine();
    let mut style = (*get_base_style()).clone();
    style.text_align = TextAlign::Center;
    let style = Arc::new(style);
    let available_width = 400.0;

    let text = "Centered text";
    let text_width = engine.measure_text_width(text, &style);

    let mut tree = IRNode::Paragraph {
        style_sets: vec![],
        style_override: None,
        children: vec![InlineNode::Text(text.to_string())],
    };

    let layout_box = layout_paragraph(&engine, &mut tree, style.clone(), (available_width, f32::INFINITY));
    if let LayoutContent::Children(lines) = layout_box.content {
        let text_run = &lines[0];
        let expected_x = (available_width - text_width) / 2.0;
        assert!((text_run.rect.x - expected_x).abs() < 0.01);
    } else {
        panic!("Paragraph layout did not produce children");
    }
}