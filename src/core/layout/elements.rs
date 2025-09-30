// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/elements.rs
//! Defines the concrete, drawable elements that are the output of the layout engine.

use crate::core::layout::ComputedStyle;
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

/// An enum representing the different types of drawable elements.
#[derive(Clone, Debug)]
pub enum LayoutElement {
    Text(TextElement),
    Rectangle(RectElement),
    Image(ImageElement),
}

impl std::fmt::Display for LayoutElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutElement::Text(t) => write!(f, "Text(\"{}\")", t.content),
            LayoutElement::Rectangle(_) => write!(f, "Rectangle"),
            LayoutElement::Image(i) => write!(f, "Image(src=\"{}\")", i.src),
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
}

/// Represents a simple rectangle, typically used for backgrounds, borders, or rules.
#[derive(Clone, Debug)]
pub struct RectElement;

/// Represents an image to be drawn.
#[derive(Clone, Debug)]
pub struct ImageElement {
    pub src: String,
}