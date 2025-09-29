
use super::style::ComputedStyle;
use super::{IRNode, LayoutBox, LayoutContent, LayoutEngine, Rect};
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
        IRNode::Root(children) | IRNode::Block { children, .. } | IRNode::List { children, .. } => {
            children
        }
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

/// Lays out a list item, adding a bullet point and indenting the children.
pub fn layout_list_item(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: Arc<ComputedStyle>,
    available_size: (f32, f32),
) -> LayoutBox {
    // TODO: List-style-type and other list properties should be driven by the style.
    const BULLET_CHAR: &str = "•";
    const BULLET_WIDTH_FACTOR: f32 = 0.6;
    const BULLET_SPACING_FACTOR: f32 = 0.4;

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

    let bullet_width = style.font_size * BULLET_WIDTH_FACTOR;
    let bullet_spacing = style.font_size * BULLET_SPACING_FACTOR;
    let indent = bullet_width + bullet_spacing;
    let child_available_size = (available_size.0 - indent, available_size.1);

    // Create the bullet as a LayoutBox.
    let bullet_box = LayoutBox {
        rect: Rect {
            x: style.padding.left,
            y: style.padding.top,
            width: bullet_width,
            height: style.line_height,
        },
        style: style.clone(),
        content: LayoutContent::Text(BULLET_CHAR.to_string(), None),
    };

    let mut child_boxes = vec![bullet_box];
    let mut content_height = 0.0;

    for child in children {
        let mut child_box = engine.build_layout_tree(child, style.clone(), child_available_size);
        child_box.rect.x += indent; // Apply indentation.
        child_box.rect.y += content_height;
        content_height += child_box.rect.height;
        child_boxes.push(child_box);
    }

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