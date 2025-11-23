use crate::core::style::border::Border;
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use crate::core::style::font::{FontStyle, FontWeight};
use crate::core::style::list::{ListStylePosition, ListStyleType};
use crate::core::style::stylesheet::ElementStyle;
use crate::core::style::text::{TextAlign, TextDecoration};
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
    pub text_decoration: TextDecoration,
    pub widows: usize,
    pub orphans: usize,
    pub color: Color,
    pub margin: Margins,
    pub padding: Margins,
    pub width: Option<Dimension>,
    pub height: Option<Dimension>,
    pub background_color: Option<Color>,
    pub border_top: Option<Border>,
    pub border_right: Option<Border>,
    pub border_bottom: Option<Border>,
    pub border_left: Option<Border>,

    // List properties
    pub list_style_type: ListStyleType,
    pub list_style_position: ListStylePosition,

    // Table properties
    pub border_spacing: f32,

    // Flexbox container properties
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: JustifyContent, // Uses same values as justify

    // Flexbox item properties
    pub order: i32,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub align_self: AlignSelf,

    // Added for compatibility with ParagraphNode expectations
    // If this property isn't in ElementStyle yet, we default it.
    pub min_height: Dimension,
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
            text_decoration: TextDecoration::None,
            widows: 2,
            orphans: 2,
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 1.0,
            },
            margin: Margins::default(),
            padding: Margins::default(),
            width: None,
            height: None,
            background_color: None,
            border_top: None,
            border_right: None,
            border_bottom: None,
            border_left: None,
            list_style_type: ListStyleType::default(),
            list_style_position: ListStylePosition::default(),
            border_spacing: 0.0,
            flex_direction: FlexDirection::default(),
            flex_wrap: FlexWrap::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            align_content: JustifyContent::FlexStart,
            order: 0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            align_self: AlignSelf::default(),
            min_height: Dimension::Auto,
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
    if style_sets.is_empty() && style_override.is_none() {
        let mut computed = (**parent_style).clone();
        computed.margin = Margins::default();
        computed.padding = Margins::default();
        computed.width = None;
        computed.height = None;
        computed.background_color = None;
        computed.border_top = None;
        computed.border_right = None;
        computed.border_bottom = None;
        computed.border_left = None;
        computed.order = 0;
        computed.flex_grow = 0.0;
        computed.flex_shrink = 1.0;
        computed.flex_basis = Dimension::Auto;
        computed.align_self = AlignSelf::Auto;
        computed.border_spacing = 0.0;
        computed.min_height = Dimension::Auto;
        return Arc::new(computed);
    }

    let mut merged = ElementStyle::default();
    for style_def in style_sets {
        merge_element_styles(&mut merged, style_def);
    }
    if let Some(override_style_def) = style_override {
        merge_element_styles(&mut merged, override_style_def);
    }

    let computed = ComputedStyle {
        // Inherited properties
        font_family: merged
            .font_family
            .map(Arc::new)
            .unwrap_or_else(|| parent_style.font_family.clone()),
        font_size: merged.font_size.unwrap_or(parent_style.font_size),
        font_weight: merged
            .font_weight
            .unwrap_or_else(|| parent_style.font_weight.clone()),
        font_style: merged
            .font_style
            .unwrap_or_else(|| parent_style.font_style.clone()),
        line_height: merged.line_height.unwrap_or_else(|| {
            merged
                .font_size
                .map(|fs| fs * 1.2)
                .unwrap_or(parent_style.line_height)
        }),
        text_align: merged
            .text_align
            .unwrap_or_else(|| parent_style.text_align.clone()),
        text_decoration: merged
            .text_decoration
            .unwrap_or_else(|| parent_style.text_decoration.clone()),
        color: merged.color.unwrap_or_else(|| parent_style.color.clone()),
        widows: merged.widows.unwrap_or(parent_style.widows),
        orphans: merged.orphans.unwrap_or(parent_style.orphans),
        list_style_type: merged
            .list_style_type
            .unwrap_or_else(|| parent_style.list_style_type.clone()),
        list_style_position: merged
            .list_style_position
            .unwrap_or_else(|| parent_style.list_style_position.clone()),
        border_spacing: merged.border_spacing.unwrap_or(parent_style.border_spacing),

        // Non-inherited properties (use merged style value or default)
        margin: merged.margin.unwrap_or_default(),
        padding: merged.padding.unwrap_or_default(),
        width: merged.width,
        height: merged.height,
        background_color: merged.background_color,

        border_top: merged.border_top.or_else(|| merged.border.clone()),
        border_right: merged.border_right.or_else(|| merged.border.clone()),
        border_bottom: merged.border_bottom.or_else(|| merged.border.clone()),
        border_left: merged.border_left.or_else(|| merged.border.clone()),

        // Flex properties (use merged style value or default from ComputedStyle)
        flex_direction: merged.flex_direction.unwrap_or_default(),
        flex_wrap: merged.flex_wrap.unwrap_or_default(),
        justify_content: merged.justify_content.unwrap_or_default(),
        align_items: merged.align_items.unwrap_or_default(),
        align_content: JustifyContent::FlexStart, // align-content is not in ElementStyle yet
        order: merged.order.unwrap_or_default(),
        flex_grow: merged.flex_grow.unwrap_or_default(),
        flex_shrink: merged.flex_shrink.unwrap_or(1.0),
        flex_basis: merged.flex_basis.unwrap_or_default(),
        align_self: merged.align_self.unwrap_or_default(),

        // Defaulting min_height since it's not in ElementStyle yet
        min_height: Dimension::Auto,
    };

    Arc::new(computed)
}

/// Returns the default style for the document root.
pub fn get_default_style() -> Arc<ComputedStyle> {
    Arc::new(ComputedStyle::default())
}

/// Merges properties from `to_apply` into `base`. If a property is `Some` in `to_apply`,
/// it overwrites the corresponding property in `base`.
fn merge_element_styles(base: &mut ElementStyle, to_apply: &ElementStyle) {
    if to_apply.font_family.is_some() {
        base.font_family = to_apply.font_family.clone();
    }
    if to_apply.font_size.is_some() {
        base.font_size = to_apply.font_size;
    }
    if to_apply.font_weight.is_some() {
        base.font_weight = to_apply.font_weight.clone();
    }
    if to_apply.font_style.is_some() {
        base.font_style = to_apply.font_style.clone();
    }
    if to_apply.line_height.is_some() {
        base.line_height = to_apply.line_height;
    }
    if to_apply.text_align.is_some() {
        base.text_align = to_apply.text_align.clone();
    }
    if to_apply.color.is_some() {
        base.color = to_apply.color.clone();
    }
    if to_apply.text_decoration.is_some() {
        base.text_decoration = to_apply.text_decoration.clone();
    }
    if to_apply.widows.is_some() {
        base.widows = to_apply.widows;
    }
    if to_apply.orphans.is_some() {
        base.orphans = to_apply.orphans;
    }
    if to_apply.background_color.is_some() {
        base.background_color = to_apply.background_color.clone();
    }
    if to_apply.border.is_some() {
        base.border = to_apply.border.clone();
    }
    if to_apply.border_top.is_some() {
        base.border_top = to_apply.border_top.clone();
    }
    if to_apply.border_right.is_some() {
        base.border_right = to_apply.border_right.clone();
    }
    if to_apply.border_bottom.is_some() {
        base.border_bottom = to_apply.border_bottom.clone();
    }
    if to_apply.border_left.is_some() {
        base.border_left = to_apply.border_left.clone();
    }
    if to_apply.margin.is_some() {
        base.margin = to_apply.margin.clone();
    }
    if to_apply.padding.is_some() {
        base.padding = to_apply.padding.clone();
    }
    if to_apply.width.is_some() {
        base.width = to_apply.width.clone();
    }
    if to_apply.height.is_some() {
        base.height = to_apply.height.clone();
    }
    if to_apply.list_style_type.is_some() {
        base.list_style_type = to_apply.list_style_type.clone();
    }
    if to_apply.list_style_position.is_some() {
        base.list_style_position = to_apply.list_style_position.clone();
    }
    if to_apply.list_style_image.is_some() {
        base.list_style_image = to_apply.list_style_image.clone();
    }
    if to_apply.border_spacing.is_some() {
        base.border_spacing = to_apply.border_spacing;
    }
    if to_apply.flex_direction.is_some() {
        base.flex_direction = to_apply.flex_direction.clone();
    }
    if to_apply.flex_wrap.is_some() {
        base.flex_wrap = to_apply.flex_wrap.clone();
    }
    if to_apply.justify_content.is_some() {
        base.justify_content = to_apply.justify_content.clone();
    }
    if to_apply.align_items.is_some() {
        base.align_items = to_apply.align_items.clone();
    }
    if to_apply.order.is_some() {
        base.order = to_apply.order;
    }
    if to_apply.flex_grow.is_some() {
        base.flex_grow = to_apply.flex_grow;
    }
    if to_apply.flex_shrink.is_some() {
        base.flex_shrink = to_apply.flex_shrink;
    }
    if to_apply.flex_basis.is_some() {
        base.flex_basis = to_apply.flex_basis.clone();
    }
    if to_apply.align_self.is_some() {
        base.align_self = to_apply.align_self.clone();
    }
}