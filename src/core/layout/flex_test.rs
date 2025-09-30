#![cfg(test)]

use crate::core::idf::IRNode;
use crate::core::style::dimension::Dimension;
use crate::core::style::flex::JustifyContent;
use crate::core::style::stylesheet::ElementStyle;
use crate::core::layout::test_utils::{create_test_engine, get_base_style};
use crate::core::layout::LayoutContent;

#[test]
fn test_flex_grow() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let container_width = 600.0;

    let mut tree = IRNode::FlexContainer {
        style_sets: vec![],
        style_override: None,
        children: vec![
            IRNode::Block {
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(100.0)),
                    flex_grow: Some(1.0),
                    ..Default::default()
                }),
                children: vec![],
            },
            IRNode::Block {
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(200.0)),
                    flex_grow: Some(2.0),
                    ..Default::default()
                }),
                children: vec![],
            },
        ],
    };

    let layout_box = engine.build_layout_tree(&mut tree, base_style, (container_width, f32::INFINITY));
    let children = match &layout_box.content {
        LayoutContent::Children(c) => c,
        _ => panic!(),
    };

    // Initial size: 100 + 200 = 300
    // Free space: 600 - 300 = 300
    // Total grow factor: 1 + 2 = 3
    // Item 1 gets: 100 + (300 * 1/3) = 200
    // Item 2 gets: 200 + (300 * 2/3) = 400
    assert!((children[0].rect.width - 200.0).abs() < 0.01);
    assert!((children[1].rect.width - 400.0).abs() < 0.01);
}

#[test]
fn test_flex_shrink() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let container_width = 300.0;

    let mut tree = IRNode::FlexContainer {
        style_sets: vec![],
        style_override: None,
        children: vec![
            IRNode::Block { // basis: 200, shrink: 1
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(200.0)),
                    flex_shrink: Some(1.0),
                    ..Default::default()
                }),
                children: vec![],
            },
            IRNode::Block { // basis: 200, shrink: 1
                style_sets: vec![],
                style_override: Some(ElementStyle {
                    width: Some(Dimension::Pt(200.0)),
                    flex_shrink: Some(1.0),
                    ..Default::default()
                }),
                children: vec![],
            },
        ],
    };

    let layout_box = engine.build_layout_tree(&mut tree, base_style, (container_width, f32::INFINITY));
    let children = match &layout_box.content {
        LayoutContent::Children(c) => c,
        _ => panic!(),
    };

    // Initial size: 200 + 200 = 400
    // Negative space: 300 - 400 = -100
    // Total shrink potential: (200 * 1) + (200 * 1) = 400
    // Item 1 shrinks by: 100 * (200*1 / 400) = 50. Final size: 200 - 50 = 150
    // Item 2 shrinks by: 100 * (200*1 / 400) = 50. Final size: 200 - 50 = 150
    assert!((children[0].rect.width - 150.0).abs() < 0.01);
    assert!((children[1].rect.width - 150.0).abs() < 0.01);
}

#[test]
fn test_flex_justify_content_space_between() {
    let engine = create_test_engine();
    let base_style = get_base_style();
    let container_width = 500.0;

    let mut tree = IRNode::FlexContainer {
        style_sets: vec![],
        style_override: Some(ElementStyle {
            justify_content: Some(JustifyContent::SpaceBetween),
            ..Default::default()
        }),
        children: vec![
            IRNode::Block {
                style_sets: vec![],
                style_override: Some(ElementStyle { width: Some(Dimension::Pt(100.0)), ..Default::default() }),
                children: vec![],
            },
            IRNode::Block {
                style_sets: vec![],
                style_override: Some(ElementStyle { width: Some(Dimension::Pt(100.0)), ..Default::default() }),
                children: vec![],
            },
        ],
    };

    let layout_box = engine.build_layout_tree(&mut tree, base_style, (container_width, f32::INFINITY));
    let children = match &layout_box.content {
        LayoutContent::Children(c) => c,
        _ => panic!(),
    };

    // Item 1 should be at the start
    assert_eq!(children[0].rect.x, 0.0);
    // Item 2 should be at the end
    assert!((children[1].rect.x - (500.0 - 100.0)).abs() < 0.01);
}