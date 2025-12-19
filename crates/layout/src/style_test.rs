use crate::style::{compute_style, get_default_style, ComputedStyle};
use petty_types::color::Color;
use petty_style::dimension::{Dimension, Margins};
use petty_style::font::FontWeight;
use petty_style::stylesheet::ElementStyle;
use petty_style::text::TextAlign;
use std::sync::Arc;

#[test]
fn test_style_inheritance() {
    let mut parent_data = get_default_style().inner.clone();
    parent_data.text.font_family = Arc::new("Times New Roman".to_string());
    parent_data.text.font_size = 20.0;
    parent_data.text.line_height = 24.0;
    parent_data.text.color = Color { r: 10, g: 20, b: 30, a: 1.0 };
    parent_data.text.text_align = TextAlign::Center;
    let parent_arc = Arc::new(ComputedStyle::new(parent_data));

    let computed = compute_style(&[], None, &parent_arc);

    assert_eq!(*computed.text.font_family, "Times New Roman");
    assert_eq!(computed.text.font_size, 20.0);
    assert_eq!(computed.text.line_height, 24.0);
    assert_eq!(computed.text.color, Color { r: 10, g: 20, b: 30, a: 1.0 });
    assert_eq!(computed.text.text_align, TextAlign::Center);
}

#[test]
fn test_style_non_inheritance() {
    let mut parent_data = get_default_style().inner.clone();
    parent_data.box_model.margin = Margins::all(50.0);
    parent_data.box_model.padding = Margins::all(30.0);
    parent_data.box_model.width = Some(Dimension::Pt(100.0));
    parent_data.misc.background_color = Some(Color { r: 255, g: 0, b: 0, a: 1.0 });
    let parent_arc = Arc::new(ComputedStyle::new(parent_data));

    let computed = compute_style(&[], None, &parent_arc);

    // Box model properties should be reset to default, not inherited.
    assert_eq!(computed.box_model.margin, Margins::default());
    assert_eq!(computed.box_model.padding, Margins::default());
    assert_eq!(computed.box_model.width, None);
    assert_eq!(computed.misc.background_color, None);
}

#[test]
fn test_line_height_auto_calculation() {
    let parent_style = get_default_style();
    let style_override = ElementStyle {
        font_size: Some(10.0),
        ..Default::default()
    };

    let computed = compute_style(&[], Some(&style_override), &parent_style);

    // line_height should be 1.2 * font_size when not specified
    assert_eq!(computed.text.font_size, 10.0);
    assert!((computed.text.line_height - 12.0).abs() < 0.01);

    // It should NOT be auto-calculated if set explicitly
    let style_override_2 = ElementStyle {
        font_size: Some(10.0),
        line_height: Some(20.0),
        ..Default::default()
    };
    let computed_2 = compute_style(&[], Some(&style_override_2), &parent_style);
    assert_eq!(computed_2.text.font_size, 10.0);
    assert_eq!(computed_2.text.line_height, 20.0);
}

#[test]
fn test_style_cascade_precedence() {
    // 1. Parent Style
    let mut parent_data = get_default_style().inner.clone();
    parent_data.text.font_size = 10.0; // P: 10
    parent_data.text.color = Color { r: 255, g: 0, b: 0, a: 1.0 }; // P: Red
    let parent_arc = Arc::new(ComputedStyle::new(parent_data));

    // 2. Named Style Set 1
    let named_style_1 = Arc::new(ElementStyle {
        font_size: Some(20.0), // N1: 20
        font_weight: Some(FontWeight::Bold), // N1: Bold
        ..Default::default()
    });

    // 3. Named Style Set 2 (should override N1 where they conflict)
    let named_style_2 = Arc::new(ElementStyle {
        font_size: Some(30.0), // N2: 30
        ..Default::default()
    });

    // 4. Inline Override (highest precedence)
    let style_override = ElementStyle {
        color: Some(Color { r: 0, g: 0, b: 255, a: 1.0 }), // I: Blue
        font_size: Some(40.0), // I: 40
        ..Default::default()
    };

    let style_sets = vec![named_style_1, named_style_2];
    let computed = compute_style(&style_sets, Some(&style_override), &parent_arc);

    // font_size is defined at all levels. Inline (40) should win.
    assert_eq!(computed.text.font_size, 40.0);

    // color is defined in Parent (Red) and Inline (Blue). Inline should win.
    assert_eq!(computed.text.color, Color { r: 0, g: 0, b: 255, a: 1.0 });

    // font_weight is only defined in Named Style 1. It should be applied.
    assert_eq!(computed.text.font_weight, FontWeight::Bold);
}