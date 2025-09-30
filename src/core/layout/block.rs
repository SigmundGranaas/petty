use super::style::ComputedStyle;
use super::{IRNode, LayoutBox, LayoutContent, LayoutEngine, Rect};
use crate::core::style::list::ListStyleType;
use std::sync::Arc;

/// Lays out a standard block container by recursively laying out its children
/// and stacking them vertically.
pub fn layout_block(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: Arc<ComputedStyle>,
    available_size: (f32, f32),
) -> LayoutBox {
    let children = match node {
        IRNode::Root(children) | IRNode::Block { children, .. } => children,
        _ => {
            return LayoutBox {
                rect: Rect::default(),
                style,
                content: LayoutContent::Children(vec![]),
            }
        }
    };

    let mut child_boxes = Vec::new();
    let mut content_height = 0.0;

    // First, lay out all children and stack them vertically.
    // Their positions are currently relative to an un-padded content box.
    for child in children {
        // For block layout, each child gets the full available width.
        let mut child_box = engine.build_layout_tree(child, style.clone(), available_size);
        // The child's `y` is already its margin-top. We just need to stack them.
        child_box.rect.y += content_height;
        content_height += child_box.rect.height;
        child_boxes.push(child_box);
    }

    // Now, offset all children by the parent's padding to move them into the padded content area.
    for child in &mut child_boxes {
        child.rect.x += style.padding.left;
        child.rect.y += style.padding.top;
    }

    LayoutBox {
        rect: Rect {
            height: content_height,
            ..Default::default()
        },
        style,
        content: LayoutContent::Children(child_boxes),
    }
}

/// Lays out a list container by recursively laying out its children
/// (ListItems) and stacking them vertically, providing them with their index.
pub fn layout_list(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: Arc<ComputedStyle>,
    available_size: (f32, f32),
) -> LayoutBox {
    let children = match node {
        IRNode::List { children, .. } => children,
        _ => {
            return LayoutBox {
                rect: Rect::default(),
                style,
                content: LayoutContent::Children(vec![]),
            }
        }
    };

    let mut child_boxes = Vec::new();
    let mut content_height = 0.0;

    for (index, child) in children.iter_mut().enumerate() {
        if let IRNode::ListItem { .. } = child {
            // We pass the index+1 because lists are 1-based.
            let mut child_box =
                layout_list_item(engine, child, style.clone(), available_size, index + 1);
            child_box.rect.y += content_height;
            content_height += child_box.rect.height;
            child_boxes.push(child_box);
        } else {
            // Non-ListItem in a List, lay out as a simple block.
            log::warn!("Found non-ListItem node inside a List. This is not recommended.");
            let mut child_box = engine.build_layout_tree(child, style.clone(), available_size);
            child_box.rect.y += content_height;
            content_height += child_box.rect.height;
            child_boxes.push(child_box);
        }
    }

    // Offset all children by the parent's padding.
    for child in &mut child_boxes {
        child.rect.x += style.padding.left;
        child.rect.y += style.padding.top;
    }

    LayoutBox {
        rect: Rect {
            height: content_height,
            ..Default::default()
        },
        style,
        content: LayoutContent::Children(child_boxes),
    }
}

/// Lays out a list item, adding a marker and indenting the children.
pub fn layout_list_item(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: Arc<ComputedStyle>,
    available_size: (f32, f32),
    index: usize,
) -> LayoutBox {
    const MARKER_SPACING_FACTOR: f32 = 0.4;

    let children = match node {
        IRNode::ListItem { children, .. } => children,
        _ => {
            return LayoutBox {
                rect: Rect::default(),
                style,
                content: LayoutContent::Children(vec![]),
            }
        }
    };

    // Determine marker content based on style.
    let marker_text = match style.list_style_type {
        ListStyleType::Disc => "•".to_string(),
        ListStyleType::Circle => "◦".to_string(),
        ListStyleType::Square => "▪".to_string(),
        ListStyleType::Decimal => format!("{}.", index),
        ListStyleType::None => String::new(),
    };

    let mut child_boxes = Vec::new();
    let indent;

    if !marker_text.is_empty() {
        let marker_width = engine.measure_text_width(&marker_text, &style);
        let marker_spacing = style.font_size * MARKER_SPACING_FACTOR;
        indent = marker_width + marker_spacing;

        let marker_box = LayoutBox {
            rect: Rect {
                x: 0.0, // Positioned relative to the list item's content box
                y: 0.0,
                width: marker_width,
                height: style.line_height,
            },
            style: style.clone(),
            content: LayoutContent::Text(marker_text, None),
        };
        child_boxes.push(marker_box);
    } else {
        indent = 0.0;
    }

    let child_available_size = (available_size.0 - indent, available_size.1);
    let mut content_height = 0.0;

    for child in children {
        let mut child_box = engine.build_layout_tree(child, style.clone(), child_available_size);
        child_box.rect.x += indent; // Apply indentation.
        child_box.rect.y += content_height;
        content_height += child_box.rect.height;
        child_boxes.push(child_box);
    }

    // Offset all children by the list item's own padding.
    for child in &mut child_boxes {
        child.rect.x += style.padding.left;
        child.rect.y += style.padding.top;
    }

    LayoutBox {
        rect: Rect {
            height: content_height.max(style.line_height),
            ..Default::default()
        },
        style,
        content: LayoutContent::Children(child_boxes),
    }
}