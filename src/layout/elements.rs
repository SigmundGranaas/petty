use crate::layout::style::ComputedStyle;

#[derive(Clone, Debug)]
pub struct PositionedElement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub element: LayoutElement,
    pub style: ComputedStyle,
}

#[derive(Clone, Debug)]
pub enum LayoutElement {
    Text(TextElement),
    Rectangle(RectElement),
}

#[derive(Clone, Debug)]
pub struct TextElement {
    pub style_name: Option<String>,
    pub content: String, // Content is a result of `join`, so it must be owned.
}

#[derive(Clone, Debug)]
pub struct RectElement {
    pub style_name: Option<String>,
}