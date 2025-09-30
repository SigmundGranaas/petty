// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/geom.rs
//! Defines basic geometric primitives for the layout engine.

/// A rectangle with position and dimensions.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}