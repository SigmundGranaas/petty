// src/layout/flex.rs

//! Layout logic for flexbox containers.

use super::style::{ComputedStyle};
use super::{ IRNode, LayoutEngine, PositionedElement};
use crate::stylesheet::Dimension;
use std::sync::Arc;

/// Lays out a flex container's children horizontally.
/// Note: This is a simplified implementation and does not handle wrapping or complex flex properties.
pub fn layout_flex_container(
    engine: &LayoutEngine,
    children: &mut [IRNode],
    style: &Arc<ComputedStyle>,
    available_width: f32,
) -> (Vec<PositionedElement>, f32, Option<super::WorkItem>) {
    let content_width = available_width - style.padding.left - style.padding.right;

    let child_widths = calculate_flex_child_widths(engine, children, content_width);
    let mut all_elements = Vec::with_capacity(children.len() * 4); // Guess: 4 elements per child on average
    let mut max_child_height = 0.0f32;
    let mut child_layouts = Vec::with_capacity(children.len());

    // First pass: lay out all children to determine their actual dimensions.
    for (i, child) in children.iter_mut().enumerate() {
        let child_width = child_widths[i];
        let (child_elements, child_height) = layout_subtree(engine, child, style, child_width);
        max_child_height = max_child_height.max(child_height);
        child_layouts.push(child_elements);
    }

    // Second pass: position the elements horizontally.
    let mut current_x = 0.0;
    for (i, mut child_elements) in child_layouts.into_iter().enumerate() {
        for el in &mut child_elements {
            el.x += current_x;
        }
        all_elements.extend(child_elements);
        current_x += child_widths[i];
    }

    // The height of the content is the height of the tallest child.
    (all_elements, max_child_height, None)
}


/// Calculates the widths of children inside a flex container.
fn calculate_flex_child_widths(
    engine: &LayoutEngine,
    children: &[IRNode],
    available_width: f32,
) -> Vec<f32> {
    if children.is_empty() {
        return vec![];
    }
    let mut widths = vec![0.0; children.len()];
    let mut auto_indices = Vec::with_capacity(children.len());
    let mut remaining_width = available_width;

    let default_parent_style = engine.get_default_style();
    for (i, child) in children.iter().enumerate() {
        let style = engine.compute_style(
            child.style_name(),
            child.style_override(),
            &default_parent_style,
        );
        if let Some(dim) = &style.width {
            match dim {
                Dimension::Pt(w) => {
                    widths[i] = *w;
                    remaining_width -= *w;
                }
                Dimension::Percent(p) => {
                    widths[i] = (p / 100.0) * available_width;
                    remaining_width -= widths[i];
                }
                _ => auto_indices.push(i),
            }
        } else {
            auto_indices.push(i);
        }
    }

    if !auto_indices.is_empty() && remaining_width > 0.0 {
        let width_per_auto = remaining_width / auto_indices.len() as f32;
        for i in auto_indices {
            widths[i] = width_per_auto;
        }
    }
    widths
}


/// Lays out a node and all its children recursively, assuming it all fits in one block.
/// This is a simplified, non-paginating layout function used for measuring items
/// within containers like flexbox or table cells.
pub(super) fn layout_subtree(
    engine: &LayoutEngine,
    node: &mut IRNode,
    parent_style: &Arc<ComputedStyle>,
    available_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let style = engine.compute_style(node.style_name(), node.style_override(), parent_style);
    let mut elements = Vec::new();
    let mut total_height = style.margin.top;

    let content_width = available_width - style.padding.left - style.padding.right;

    let (content_elements, content_height) = match node {
        IRNode::Paragraph { children, .. } => {
            // Since this is for measurement, we assume infinite vertical space.
            let (els, height, _remainder) =
                super::text::layout_paragraph(engine, children, &style, content_width, f32::MAX);
            (els, height + style.padding.top + style.padding.bottom)
        }
        IRNode::Root(children)
        | IRNode::Block { children, .. }
        | IRNode::List { children, .. } => { // List is also a simple vertical container
            let mut block_elements = Vec::with_capacity(children.len() * 4);
            let mut current_y = style.padding.top;
            for child in children {
                let (mut child_elements, child_height) =
                    layout_subtree(engine, child, &style, content_width);
                for el in &mut child_elements {
                    el.y += current_y;
                }
                block_elements.extend(child_elements);
                current_y += child_height;
            }
            (block_elements, current_y + style.padding.bottom)
        }
        IRNode::ListItem { children, .. } => {
            let bullet_width = style.font_size * 0.6;
            let bullet_spacing = style.font_size * 0.4;
            let bullet = PositionedElement {
                x: 0.0,
                y: style.padding.top,
                width: bullet_width,
                height: style.line_height,
                element: super::elements::LayoutElement::Text(super::elements::TextElement {
                    content: "•".to_string(),
                    href: None,
                }),
                style: style.clone(),
            };

            // PERF: Pre-allocate vector with a reasonable guess.
            let mut all_elements = Vec::with_capacity(1 + children.len() * 4);
            all_elements.push(bullet);

            let mut current_y = style.padding.top;

            let mut indented_style_arc = style.clone();
            let indented_style_mut = Arc::make_mut(&mut indented_style_arc);
            indented_style_mut.padding.left += bullet_width + bullet_spacing;

            for child in children {
                // The child's layout will account for its new, larger padding.
                let (mut child_elements, child_height) =
                    layout_subtree(engine, child, &indented_style_arc, content_width);

                for el in &mut child_elements {
                    el.y += current_y;
                }
                all_elements.extend(child_elements);
                current_y += child_height;
            }

            // The total height of the children content.
            let children_content_height = current_y - style.padding.top;
            // The list item's content height must be at least one line height for the bullet.
            let final_content_height = children_content_height.max(style.line_height);

            (all_elements, final_content_height + style.padding.top + style.padding.bottom)
        }
        IRNode::FlexContainer { children, .. } => {
            let (els, height, _remainder) =
                layout_flex_container(engine, children, &style, content_width);
            (els, height + style.padding.top + style.padding.bottom)
        }
        IRNode::Image { src, data, .. } => {
            let (els, height, _remainder) =
                super::image::layout_image(src, data.as_ref(), &style, content_width);
            (els, height + style.padding.top + style.padding.bottom)
        }
        IRNode::Table { header, body, calculated_widths, .. } => {
            let (els, height, _remainder) =
                super::table::layout_table(engine, header.as_deref_mut(), body, &style, calculated_widths, f32::MAX);
            (els, height + style.padding.top + style.padding.bottom)
        }
    };

    for mut el in content_elements {
        el.y += total_height;
        el.x += style.padding.left;
        elements.push(el);
    }
    total_height += content_height + style.margin.bottom;

    (elements, total_height)
}

/// A cheap, measurement-only version of `layout_subtree`.
/// It calculates the height of a node and its children without allocating any `PositionedElement`s.
pub(super) fn measure_subtree_height(
    engine: &LayoutEngine,
    node: &mut IRNode,
    parent_style: &Arc<ComputedStyle>,
    available_width: f32,
) -> f32 {
    let style = engine.compute_style(node.style_name(), node.style_override(), parent_style);
    let mut total_height = style.margin.top;

    let content_width = available_width - style.padding.left - style.padding.right;

    let content_height = match node {
        IRNode::Paragraph { children, .. } => {
            let height = super::text::measure_paragraph_height(engine, children, &style, content_width);
            height + style.padding.top + style.padding.bottom
        }
        IRNode::Root(children) | IRNode::Block { children, .. } | IRNode::List { children, .. } => {
            let mut current_y = style.padding.top;
            for child in children {
                let child_height =
                    measure_subtree_height(engine, child, &style, content_width);
                current_y += child_height;
            }
            current_y + style.padding.bottom
        }
        IRNode::ListItem { children, .. } => {
            let bullet_width = style.font_size * 0.6;
            let bullet_spacing = style.font_size * 0.4;
            let mut current_y = style.padding.top;

            let mut indented_style_arc = style.clone();
            let indented_style_mut = Arc::make_mut(&mut indented_style_arc);
            indented_style_mut.padding.left += bullet_width + bullet_spacing;


            for child in children {
                let child_height =
                    measure_subtree_height(engine, child, &indented_style_arc, content_width);
                current_y += child_height;
            }

            let children_content_height = current_y - style.padding.top;
            let final_content_height = children_content_height.max(style.line_height);

            final_content_height + style.padding.top + style.padding.bottom
        }
        IRNode::FlexContainer { children, .. } => {
            let child_widths = calculate_flex_child_widths(engine, children, content_width);
            let mut max_child_height = 0.0f32;

            for (i, child) in children.iter_mut().enumerate() {
                let child_width = child_widths[i];
                let child_height = measure_subtree_height(engine, child, &style, child_width);
                max_child_height = max_child_height.max(child_height);
            }
            max_child_height + style.padding.top + style.padding.bottom
        }
        IRNode::Image { data, .. } => {
            let height = if data.is_some() {
                style.height.unwrap_or(50.0)
            } else {
                0.0
            };
            height + style.padding.top + style.padding.bottom
        }
        IRNode::Table { header, body, calculated_widths, .. } => {
            let measure_row = |row: &mut crate::idf::TableRow| -> f32 {
                let mut max_cell_height: f32 = 0.0;
                for (i, cell) in row.cells.iter_mut().enumerate() {
                    let cell_width = *calculated_widths.get(i).unwrap_or(&0.0);
                    let cell_style = engine.compute_style(
                        cell.style_name.as_deref(),
                        cell.style_override.as_ref(),
                        &style,
                    );

                    let mut cell_root = IRNode::Root(std::mem::take(&mut cell.children));
                    let cell_height =
                        measure_subtree_height(engine, &mut cell_root, &cell_style, cell_width);

                    if let IRNode::Root(children) = cell_root {
                        cell.children = children;
                    }

                    max_cell_height = max_cell_height.max(cell_height);
                }
                max_cell_height
            };

            let mut table_content_height = 0.0;
            if let Some(h) = header.as_deref_mut() {
                for row in &mut h.rows {
                    table_content_height += measure_row(row);
                }
            }
            for row in &mut body.rows {
                table_content_height += measure_row(row);
            }

            table_content_height + style.padding.top + style.padding.bottom
        }
    };

    total_height += content_height + style.margin.bottom;
    total_height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::{InlineNode, IRNode};
    use crate::layout::elements::LayoutElement;
    use crate::layout::engine::LayoutEngine;
    use crate::stylesheet::{Dimension, ElementStyle, Stylesheet};
    use std::collections::HashMap;

    fn create_test_engine() -> LayoutEngine {
        let mut styles = HashMap::new();
        styles.insert(
            "w50".to_string(),
            ElementStyle {
                width: Some(Dimension::Pt(50.0)),
                ..Default::default()
            },
        );
        styles.insert(
            "p50".to_string(),
            ElementStyle {
                width: Some(Dimension::Percent(50.0)),
                ..Default::default()
            },
        );
        let stylesheet = Stylesheet {
            styles,
            ..Default::default()
        };
        LayoutEngine::new(stylesheet)
    }

    #[test]
    fn test_flex_child_widths_mixed() {
        let engine = create_test_engine();
        let children = vec![
            IRNode::Block {
                style_name: Some("w50".to_string()),
                style_override: None,
                children: vec![],
            },
            IRNode::Block {
                style_name: Some("p50".to_string()),
                style_override: None,
                children: vec![],
            },
            IRNode::Block {
                style_name: None,
                style_override: None,
                children: vec![],
            }, // auto
        ];
        // available_width = 300
        // w50 takes 50pt. Remaining = 250.
        // p50 takes 50% of original 300 = 150. Remaining = 100.
        // auto takes the final 100.
        let widths = calculate_flex_child_widths(&engine, &children, 300.0);
        assert_eq!(widths, vec![50.0, 150.0, 100.0]);
    }

    #[test]
    fn test_layout_subtree_handles_list_and_listitem() {
        let engine = create_test_engine();
        let default_style = engine.get_default_style();
        let mut tree = IRNode::List {
            style_name: None,
            style_override: None,
            children: vec![IRNode::ListItem {
                style_name: None,
                style_override: None,
                children: vec![IRNode::Paragraph {
                    style_name: None,
                    style_override: None,
                    children: vec![InlineNode::Text("Item 1".into())],
                }],
            }],
        };

        // This call would panic or return incorrect results with the old implementation
        let (elements, height) = layout_subtree(&engine, &mut tree, &default_style, 500.0);

        // Expected elements: 1 bullet + 1 text line = 2 elements
        assert_eq!(
            elements.len(),
            2,
            "Should have a bullet and text for the list item"
        );

        // Verify that the text element is indented to make room for the bullet
        let text_el = elements
            .iter()
            .find(|e| matches!(&e.element, LayoutElement::Text(t) if t.content == "Item 1"))
            .unwrap();
        let bullet_el = elements
            .iter()
            .find(|e| matches!(&e.element, LayoutElement::Text(t) if t.content == "•"))
            .unwrap();

        let bullet_width = default_style.font_size * 0.6;
        let bullet_spacing = default_style.font_size * 0.4;
        let expected_indent = bullet_width + bullet_spacing;

        // The bullet should be at the start of the list item's content box
        assert!((bullet_el.x).abs() < 0.1);
        // The text's X position is determined by the padding of its parent paragraph,
        // which was increased by layout_subtree.
        assert!(
            (text_el.x - expected_indent).abs() < 0.1,
            "Text should be indented past the bullet"
        );
        assert!(
            height >= default_style.line_height,
            "The measured height should be at least one line high"
        );
    }
}