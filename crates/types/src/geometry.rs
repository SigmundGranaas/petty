#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

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
        Self {
            width: 0.0,
            height: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoxConstraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}

impl PartialEq for BoxConstraints {
    fn eq(&self, other: &Self) -> bool {
        const EPSILON: f32 = 0.01;
        (self.min_width - other.min_width).abs() < EPSILON
            && (self.max_width - other.max_width).abs() < EPSILON
            && (self.min_height - other.min_height).abs() < EPSILON
            && (self.max_height - other.max_height).abs() < EPSILON
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

    pub fn tight(size: Size) -> Self {
        Self {
            min_width: size.width,
            max_width: size.width,
            min_height: size.height,
            max_height: size.height,
        }
    }

    pub fn tight_width(width: f32) -> Self {
        Self {
            min_width: width,
            max_width: width,
            min_height: 0.0,
            max_height: f32::INFINITY,
        }
    }

    pub fn loose(size: Size) -> Self {
        Self {
            min_width: 0.0,
            max_width: size.width,
            min_height: 0.0,
            max_height: size.height,
        }
    }

    pub fn has_bounded_width(self) -> bool {
        self.max_width.is_finite()
    }

    pub fn has_bounded_height(self) -> bool {
        self.max_height.is_finite()
    }

    pub fn is_tight(self) -> bool {
        self.min_width >= self.max_width && self.min_height >= self.max_height
    }

    pub fn constrain(self, size: Size) -> Size {
        Size {
            width: size.width.clamp(self.min_width, self.max_width),
            height: size.height.clamp(self.min_height, self.max_height),
        }
    }

    pub fn constrain_width(self, width: f32) -> f32 {
        width.clamp(self.min_width, self.max_width)
    }

    pub fn constrain_height(self, height: f32) -> f32 {
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
