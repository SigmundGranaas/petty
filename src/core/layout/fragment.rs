//! Defines the `LayoutBox`, the primary data structure for the geometry tree.

use std::sync::Arc;
use crate::core::layout::style::ComputedStyle;

/// A rectangle with position and dimensions.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The output of the layout-pass. It's a geometry-aware tree where each node
/// has a resolved size and position relative to its parent. This tree is then
/// consumed by the pagination pass.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// The rectangle defining the box's position and dimensions, relative to its parent.
    pub rect: Rect,
    /// The fully computed style for this box.
    pub style: Arc<ComputedStyle>,
    /// The content of this box.
    pub content: LayoutContent,
}

/// The different types of content a `LayoutBox` can contain.
#[derive(Debug, Clone)]
pub enum LayoutContent {
    /// A box that contains other boxes (e.g., a `Block` or `FlexContainer`).
    Children(Vec<LayoutBox>),
    /// A box containing a block of text.
    Text(String, Option<String>), // content, href
    /// A box containing an image.
    Image(String), // src
    /// A simple colored rectangle, used for backgrounds.
    Color,
    /// A placeholder for content that has been paginated and will be handled on a future page.
    Pending,
}