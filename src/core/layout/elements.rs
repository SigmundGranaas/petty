// src/core/layout/elements.rs

use crate::core::base::geometry;
use crate::core::layout::style::ComputedStyle;
use crate::core::style::text::TextDecoration;
use std::sync::Arc;

/// A simple, geometry-aware data structure representing a single drawable item.
/// This is the final output of the layout process for a given element, containing
/// its absolute position and final styling information. A page is simply a collection
/// of these elements.
#[derive(Clone, Debug)]
pub struct PositionedElement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub element: LayoutElement,
    pub style: Arc<ComputedStyle>,
}

impl PositionedElement {
    /// Creates a partial `PositionedElement` from a `Rect`.
    /// The `element` and `style` fields must be filled in by the caller.
    pub fn from_rect(rect: geometry::Rect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            element: LayoutElement::Rectangle(RectElement), // Placeholder
            style: Arc::new(ComputedStyle::default()),      // Placeholder
        }
    }
}

/// An enum representing the different types of drawable elements.
#[derive(Clone, Debug)]
pub enum LayoutElement {
    Text(TextElement),
    Rectangle(RectElement),
    Image(ImageElement),
    PageNumberPlaceholder {
        target_id: String,
        href: Option<String>,
    },
}

impl std::fmt::Display for LayoutElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutElement::Text(t) => write!(f, "Text(\"{}\")", t.content),
            LayoutElement::Rectangle(_) => write!(f, "Rectangle"),
            LayoutElement::Image(i) => write!(f, "Image(src=\"{}\")", i.src),
            LayoutElement::PageNumberPlaceholder { target_id, .. } => {
                write!(f, "PageNumberPlaceholder(target=\"{}\")", target_id)
            }
        }
    }
}

/// Represents a block of text to be drawn.
#[derive(Clone, Debug)]
pub struct TextElement {
    /// The final, wrapped text content. May contain newlines.
    pub content: String,
    /// If present, this text is a hyperlink to the given URL.
    pub href: Option<String>,
    /// Specifies any decoration, like an underline.
    pub text_decoration: TextDecoration,
}

/// Represents a simple rectangle, typically used for backgrounds, borders, or rules.
#[derive(Clone, Debug)]
pub struct RectElement;

/// Represents an image to be drawn.
#[derive(Clone, Debug)]
pub struct ImageElement {
    pub src: String,
}