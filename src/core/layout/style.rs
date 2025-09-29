use crate::core::style::border::Border;
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins, PageSize};
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use crate::core::style::font::{FontStyle, FontWeight};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::core::style::text::TextAlign;
use std::sync::Arc;

/// A fully resolved style with no optional values, ready for layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    pub font_family: Arc<String>,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub color: Color,
    pub margin: Margins,
    pub padding: Margins,
    pub width: Option<Dimension>,
    pub height: Option<Dimension>,
    pub background_color: Option<Color>,
    pub border: Option<Border>,
    pub border_bottom: Option<Border>,

    // Flexbox container properties
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,

    // Flexbox item properties
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub align_self: AlignSelf,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            font_family: Arc::new("Helvetica".to_string()),
            font_size: 12.0,
            font_weight: FontWeight::Regular,
            font_style: FontStyle::Normal,
            line_height: 14.4, // 12.0 * 1.2
            text_align: TextAlign::Left,
            color: Color { r: 0, g: 0, b: 0, a: 1.0 },
            margin: Margins::default(),
            padding: Margins::default(),
            width: None,
            height: None,
            background_color: None,
            border: None,
            border_bottom: None,
            flex_direction: FlexDirection::default(),
            flex_wrap: FlexWrap::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            align_self: AlignSelf::default(),
        }
    }
}

/// Computes the style for a node by inheriting from its parent, applying any named
/// style from the stylesheet, and finally applying any inline style overrides.
pub fn compute_style(
    style_sets: &[Arc<ElementStyle>],
    style_override: Option<&ElementStyle>,
    parent_style: &Arc<ComputedStyle>,
) -> Arc<ComputedStyle> {
    // 1. Create a mutable copy of the parent style to inherit properties like font, color, etc.
    let mut computed = (**parent_style).clone();

    // 2. Reset non-inherited CSS properties to their default values.
    // This prevents inheriting things like padding, margin, width, etc.
    computed.margin = Margins::default();
    computed.padding = Margins::default();
    computed.width = None;
    computed.height = None;
    computed.background_color = None;
    computed.border = None;
    computed.border_bottom = None;
    computed.flex_grow = 0.0;
    computed.flex_shrink = 1.0;
    computed.flex_basis = Dimension::Auto;
    computed.align_self = AlignSelf::Auto;

    // 3. Apply all pre-resolved named styles in order.
    for style_def in style_sets {
        apply_element_style(&mut computed, style_def.as_ref());
    }

    // 4. Apply the inline style override, which has the highest precedence.
    if let Some(override_style_def) = style_override {
        apply_element_style(&mut computed, override_style_def);
    }

    Arc::new(computed)
}

/// Returns the default style for the document root.
pub fn get_default_style() -> Arc<ComputedStyle> {
    Arc::new(ComputedStyle::default())
}

/// Returns the page dimensions in points based on the stylesheet.
pub fn get_page_dimensions(stylesheet: &Stylesheet) -> (f32, f32) {
    match stylesheet.page.size {
        PageSize::A4 => (595.0, 842.0),
        PageSize::Letter => (612.0, 792.0),
        PageSize::Legal => (612.0, 1008.0),
        PageSize::Custom { width, height } => (width, height),
    }
}

/// Applies style rules from an `ElementStyle` definition to a `ComputedStyle`.
fn apply_element_style(computed: &mut ComputedStyle, style_def: &ElementStyle) {
    if let Some(ff) = &style_def.font_family {
        computed.font_family = Arc::new(ff.clone());
    }
    if let Some(fs) = style_def.font_size {
        computed.font_size = fs;
        if style_def.line_height.is_none() {
            computed.line_height = fs * 1.2;
        }
    }
    if let Some(fw) = &style_def.font_weight {
        computed.font_weight = fw.clone();
    }
    if let Some(fs) = &style_def.font_style {
        computed.font_style = fs.clone();
    }
    if let Some(lh) = style_def.line_height {
        computed.line_height = lh;
    }
    if let Some(ta) = &style_def.text_align {
        computed.text_align = ta.clone();
    }
    if let Some(c) = &style_def.color {
        computed.color = c.clone();
    }
    if let Some(m) = &style_def.margin {
        computed.margin = m.clone();
    }
    if let Some(p) = &style_def.padding {
        computed.padding = p.clone();
    }
    if let Some(w) = &style_def.width {
        computed.width = Some(w.clone())
    }
    if let Some(h) = &style_def.height {
        computed.height = Some(h.clone());
    }
    if let Some(bg) = &style_def.background_color {
        computed.background_color = Some(bg.clone());
    }
    if let Some(b) = &style_def.border {
        computed.border = Some(b.clone());
    }
    if let Some(b) = &style_def.border_bottom {
        computed.border_bottom = Some(b.clone());
    }
    if let Some(d) = &style_def.flex_direction {
        computed.flex_direction = d.clone();
    }
    if let Some(w) = &style_def.flex_wrap {
        computed.flex_wrap = w.clone();
    }
    if let Some(jc) = &style_def.justify_content {
        computed.justify_content = jc.clone();
    }
    if let Some(ai) = &style_def.align_items {
        computed.align_items = ai.clone();
    }
    if let Some(g) = style_def.flex_grow {
        computed.flex_grow = g;
    }
    if let Some(s) = style_def.flex_shrink {
        computed.flex_shrink = s;
    }
    if let Some(b) = &style_def.flex_basis {
        computed.flex_basis = b.clone();
    }
    if let Some(s) = &style_def.align_self {
        computed.align_self = s.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::style::color::Color;
    use crate::core::style::stylesheet::ElementStyle;

    #[test]
    fn test_default_style() {
        let default = get_default_style();
        assert_eq!(default.font_size, 12.0);
        assert_eq!(default.line_height, 14.4);
    }

    #[test]
    fn test_style_application() {
        let parent_style = get_default_style();
        let style_set = vec![Arc::new(ElementStyle {
            font_size: Some(20.0),
            color: Some(Color {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0,
            }),
            ..Default::default()
        })];

        let computed = compute_style(&style_set, None, &parent_style);

        assert_eq!(computed.font_size, 20.0);
        assert_eq!(computed.line_height, 24.0); // 20.0 * 1.2
        assert_eq!(computed.color.r, 255);
        assert_eq!(*computed.font_family, "Helvetica"); // Inherited
    }
}