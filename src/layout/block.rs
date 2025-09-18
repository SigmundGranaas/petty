// src/layout/block.rs

//! Layout logic for block-level containers like `Block`, `List`, and `ListItem`.

use super::elements::{LayoutElement, PositionedElement, RectElement, TextElement};
use super::style::ComputedStyle;
use super::{IRNode, WorkItem};

/// Lays out a standard block container by pushing its children onto the work stack.
/// The actual height is determined as the children are processed.
pub fn layout_block(
    work_stack: &mut Vec<(WorkItem, ComputedStyle)>,
    children: &mut [IRNode],
    style: &ComputedStyle,
) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
    work_stack.push((WorkItem::EndNode(style.clone()), style.clone()));
    for child in children.iter().rev() {
        work_stack.push((WorkItem::Node(child.clone()), style.clone()));
    }
    (vec![], 0.0, None)
}

/// Lays out a list item, adding a bullet point and indenting the children.
pub fn layout_list_item(
    work_stack: &mut Vec<(WorkItem, ComputedStyle)>,
    children: &mut [IRNode],
    style: &ComputedStyle,
) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
    // Place bullet at the start of the content box.
    // The list item itself should have a margin/padding from the XSLT to indent the whole thing.
    let bullet_width = style.font_size * 0.6; // "•" approx
    let bullet_spacing = style.font_size * 0.4;
    let bullet = PositionedElement {
        x: 0.0, // Start of the content box.
        y: style.padding.top, // Align with first line of text
        width: bullet_width,
        height: style.line_height,
        element: LayoutElement::Text(TextElement {
            content: "•".to_string(),
            href: None,
        }),
        style: style.clone(),
    };

    work_stack.push((WorkItem::EndNode(style.clone()), style.clone()));
    for child in children.iter().rev() {
        let mut indented_style = style.clone();
        // Add padding to the children to clear the bullet.
        indented_style.padding.left += bullet_width + bullet_spacing;
        work_stack.push((WorkItem::Node(child.clone()), indented_style));
    }

    (vec![bullet], 0.0, None)
}


/// Prepends a background rectangle to a list of elements.
pub fn add_background(
    elements: &mut Vec<PositionedElement>,
    style: &ComputedStyle,
    width: f32,
    height: f32,
) {
    elements.insert(
        0,
        PositionedElement {
            x: style.padding.left,
            y: style.padding.top,
            width: width - style.padding.left - style.padding.right,
            height: height - style.padding.top - style.padding.bottom,
            element: LayoutElement::Rectangle(RectElement),
            style: style.clone(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::InlineNode;
    use crate::layout::style::get_default_style;
    use crate::layout::WorkItem;

    #[test]
    fn test_layout_block_pushes_children_to_stack() {
        let mut work_stack: Vec<(WorkItem, ComputedStyle)> = Vec::new();
        let style = get_default_style();

        let mut children = [
            IRNode::Paragraph {
                style_name: Some("p1".to_string()),
                style_override: None,
                children: vec![],
            },
            IRNode::Paragraph {
                style_name: Some("p2".to_string()),
                style_override: None,
                children: vec![],
            },
        ];

        layout_block(&mut work_stack, &mut children, &style);

        // Expected stack (from top to bottom): Node(p1), Node(p2), EndNode
        // This is because children are pushed in reverse order to be processed in document order.
        assert_eq!(work_stack.len(), 3);

        // Check Node(p1) - it was pushed last, so it's on top of the stack
        let (item1, _) = work_stack.pop().unwrap();
        if let WorkItem::Node(IRNode::Paragraph { style_name, .. }) = item1 {
            assert_eq!(style_name, Some("p1".to_string()));
        } else {
            panic!("Expected Node p1");
        }

        // Check Node(p2) - it was pushed before p1
        let (item2, _) = work_stack.pop().unwrap();
        if let WorkItem::Node(IRNode::Paragraph { style_name, .. }) = item2 {
            assert_eq!(style_name, Some("p2".to_string()));
        } else {
            panic!("Expected Node p2");
        }

        // Check EndNode - it was pushed first
        let (item3, _) = work_stack.pop().unwrap();
        assert!(matches!(item3, WorkItem::EndNode(_)));
    }

    #[test]
    fn test_layout_list_item_creates_bullet_and_indents() {
        let mut work_stack: Vec<(WorkItem, ComputedStyle)> = Vec::new();
        let mut style = get_default_style();
        style.padding.left = 10.0;
        let original_padding = style.padding.left;

        let mut children = [IRNode::Paragraph {
            style_name: None,
            style_override: None,
            children: vec![InlineNode::Text("Item content".to_string())],
        }];

        let (elements, _, _) = layout_list_item(&mut work_stack, &mut children, &style);

        // Check for bullet point element
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].x, 0.0); // Bullet is at the start of the item's content box
        if let LayoutElement::Text(text_el) = &elements[0].element {
            assert_eq!(text_el.content, "•");
        } else {
            panic!("Expected a text element for the bullet");
        }

        // Check that the child on the work stack has an indented style
        assert_eq!(work_stack.len(), 2); // Child Node + EndNode
        let (child_item, indented_style) = &work_stack[1]; // The child is second from top
        assert!(matches!(child_item, WorkItem::Node(_)));

        let bullet_width = style.font_size * 0.6;
        let bullet_spacing = style.font_size * 0.4;
        assert_eq!(indented_style.padding.left, original_padding + bullet_width + bullet_spacing);
    }
}