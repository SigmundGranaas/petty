// FILE: /home/sigmund/RustroverProjects/petty/src/templating/style.rs
use crate::core::style::border::Border;
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::flex::{AlignItems, FlexDirection, FlexWrap, JustifyContent};
use crate::core::style::font::FontWeight;
use crate::core::style::stylesheet::ElementStyle;
use crate::core::style::text::TextAlign;

/// The core trait for adding inline styles fluently.
pub trait StyledWidget: Sized {
    fn style_override_mut(&mut self) -> &mut ElementStyle;

    fn font_size(mut self, size: f32) -> Self {
        self.style_override_mut().font_size = Some(size);
        self
    }

    fn font_weight(mut self, weight: FontWeight) -> Self {
        self.style_override_mut().font_weight = Some(weight);
        self
    }

    fn color(mut self, color: Color) -> Self {
        self.style_override_mut().color = Some(color);
        self
    }

    fn text_align(mut self, align: TextAlign) -> Self {
        self.style_override_mut().text_align = Some(align);
        self
    }

    fn padding(mut self, margins: Margins) -> Self {
        self.style_override_mut().padding = Some(margins);
        self
    }

    fn margin(mut self, margins: Margins) -> Self {
        self.style_override_mut().margin = Some(margins);
        self
    }

    fn width(mut self, width: Dimension) -> Self {
        self.style_override_mut().width = Some(width);
        self
    }

    fn height(mut self, height: Dimension) -> Self {
        self.style_override_mut().height = Some(height);
        self
    }

    fn background_color(mut self, color: Color) -> Self {
        self.style_override_mut().background_color = Some(color);
        self
    }

    fn border(mut self, border: Border) -> Self {
        self.style_override_mut().border = Some(border);
        self
    }

    fn border_top(mut self, border: Border) -> Self {
        self.style_override_mut().border_top = Some(border);
        self
    }

    fn border_right(mut self, border: Border) -> Self {
        self.style_override_mut().border_right = Some(border);
        self
    }

    fn border_bottom(mut self, border: Border) -> Self {
        self.style_override_mut().border_bottom = Some(border);
        self
    }

    fn border_left(mut self, border: Border) -> Self {
        self.style_override_mut().border_left = Some(border);
        self
    }

    fn flex_grow(mut self, value: f32) -> Self {
        self.style_override_mut().flex_grow = Some(value);
        self
    }

    // --- NEW: Flex Container Properties ---
    fn flex_direction(mut self, direction: FlexDirection) -> Self {
        self.style_override_mut().flex_direction = Some(direction);
        self
    }

    fn flex_wrap(mut self, wrap: FlexWrap) -> Self {
        self.style_override_mut().flex_wrap = Some(wrap);
        self
    }

    fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.style_override_mut().justify_content = Some(justify);
        self
    }

    fn align_items(mut self, align: AlignItems) -> Self {
        self.style_override_mut().align_items = Some(align);
        self
    }
}

/// A macro to easily implement the trait for any builder struct that has
/// a `style_override: ElementStyle` field.
macro_rules! impl_styled_widget {
    ($($t:ty),+) => {
        $(
            impl $crate::templating::style::StyledWidget for $t {
                fn style_override_mut(&mut self) -> &mut $crate::core::style::stylesheet::ElementStyle {
                    &mut self.style_override
                }
            }
        )+
    };
}

// Make the macro available to other modules
pub(crate) use impl_styled_widget;