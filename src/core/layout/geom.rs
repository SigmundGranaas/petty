/// A rectangle with position and dimensions.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A 2D size.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn zero() -> Self {
        Self { width: 0.0, height: 0.0 }
    }
}

/// Layout constraints for a node.
/// Modeled after Flutter's BoxConstraints.
#[derive(Debug, Clone, Copy)]
pub struct BoxConstraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}

impl PartialEq for BoxConstraints {
    fn eq(&self, other: &Self) -> bool {
        let eps = 0.001;
        (self.min_width - other.min_width).abs() < eps
            && (self.max_width - other.max_width).abs() < eps
            && (self.min_height - other.min_height).abs() < eps
            && (self.max_height - other.max_height).abs() < eps
    }
}

impl BoxConstraints {
    pub fn new(min_width: f32, max_width: f32, min_height: f32, max_height: f32) -> Self {
        Self {
            min_width,
            max_width,
            min_height,
            max_height,
        }
    }

    /// Creates constraints that require the size to be exactly the given dimensions.
    pub fn tight(size: Size) -> Self {
        Self {
            min_width: size.width,
            max_width: size.width,
            min_height: size.height,
            max_height: size.height,
        }
    }

    /// Creates constraints that require the width to be exactly `width`, but height is unconstrained (0 to infinity).
    pub fn tight_width(width: f32) -> Self {
        Self {
            min_width: width,
            max_width: width,
            min_height: 0.0,
            max_height: f32::INFINITY,
        }
    }

    /// Creates loose constraints (0 to size).
    pub fn loose(size: Size) -> Self {
        Self {
            min_width: 0.0,
            max_width: size.width,
            min_height: 0.0,
            max_height: size.height,
        }
    }

    /// Returns true if the width constraint is finite.
    pub fn has_bounded_width(&self) -> bool {
        self.max_width.is_finite()
    }

    /// Returns true if the height constraint is finite.
    pub fn has_bounded_height(&self) -> bool {
        self.max_height.is_finite()
    }

    /// Returns true if the constraints require a specific size.
    pub fn is_tight(&self) -> bool {
        self.min_width >= self.max_width && self.min_height >= self.max_height
    }

    /// Constrains a size to fit within these constraints.
    pub fn constrain(&self, size: Size) -> Size {
        Size {
            width: size.width.clamp(self.min_width, self.max_width),
            height: size.height.clamp(self.min_height, self.max_height),
        }
    }

    /// Constrains a width to fit within these constraints.
    pub fn constrain_width(&self, width: f32) -> f32 {
        width.clamp(self.min_width, self.max_width)
    }

    /// Constrains a height to fit within these constraints.
    pub fn constrain_height(&self, height: f32) -> f32 {
        height.clamp(self.min_height, self.max_height)
    }
}

impl Default for BoxConstraints {
    fn default() -> Self {
        Self {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        }
    }
}