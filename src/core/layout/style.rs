use crate::core::style::border::Border;
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use crate::core::style::font::{FontStyle, FontWeight};
use crate::core::style::list::{ListStylePosition, ListStyleType};
use crate::core::style::stylesheet::ElementStyle;
use crate::core::style::text::{TextAlign, TextDecoration};
use crate::core::layout::geom::BoxConstraints;
use std::sync::Arc;

// Grouped Style Structures

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoxModel {
    pub margin: Margins,
    pub padding: Margins,
    pub width: Option<Dimension>,
    pub height: Option<Dimension>,
    pub min_height: Dimension,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BorderModel {
    pub top: Option<Border>,
    pub right: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextModel {
    pub font_family: Arc<String>,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub text_decoration: TextDecoration,
    pub color: Color,
}

impl Default for TextModel {
    fn default() -> Self {
        Self {
            font_family: Arc::new("Helvetica".to_string()),
            font_size: 12.0,
            font_weight: FontWeight::Regular,
            font_style: FontStyle::Normal,
            line_height: 14.4,
            text_align: TextAlign::Left,
            text_decoration: TextDecoration::None,
            color: Color::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlexModel {
    pub direction: FlexDirection,
    pub wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: JustifyContent,
    // Item properties
    pub order: i32,
    pub grow: f32,
    pub shrink: f32,
    pub basis: Dimension,
    pub align_self: AlignSelf,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ListModel {
    pub style_type: ListStyleType,
    pub style_position: ListStylePosition,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableModel {
    pub border_spacing: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MiscModel {
    pub widows: usize,
    pub orphans: usize,
    pub background_color: Option<Color>,
}

impl Default for MiscModel {
    fn default() -> Self {
        Self {
            widows: 2,
            orphans: 2,
            background_color: None,
        }
    }
}

/// A fully resolved style with grouped properties.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ComputedStyle {
    pub box_model: BoxModel,
    pub border: BorderModel,
    pub text: TextModel,
    pub flex: FlexModel,
    pub list: ListModel,
    pub table: TableModel,
    pub misc: MiscModel,
}

impl ComputedStyle {
    /// Returns the total width of horizontal padding.
    pub fn padding_x(&self) -> f32 {
        self.box_model.padding.left + self.box_model.padding.right
    }

    /// Returns the total height of vertical padding.
    pub fn padding_y(&self) -> f32 {
        self.box_model.padding.top + self.box_model.padding.bottom
    }

    /// Returns the total width of horizontal borders.
    pub fn border_x(&self) -> f32 {
        self.border.left.as_ref().map_or(0.0, |b| b.width) +
            self.border.right.as_ref().map_or(0.0, |b| b.width)
    }

    /// Returns the total height of vertical borders.
    pub fn border_y(&self) -> f32 {
        self.border.top.as_ref().map_or(0.0, |b| b.width) +
            self.border.bottom.as_ref().map_or(0.0, |b| b.width)
    }

    pub fn border_top_width(&self) -> f32 {
        self.border.top.as_ref().map_or(0.0, |b| b.width)
    }

    pub fn border_bottom_width(&self) -> f32 {
        self.border.bottom.as_ref().map_or(0.0, |b| b.width)
    }

    pub fn border_left_width(&self) -> f32 {
        self.border.left.as_ref().map_or(0.0, |b| b.width)
    }

    pub fn border_right_width(&self) -> f32 {
        self.border.right.as_ref().map_or(0.0, |b| b.width)
    }

    /// Calculates constraints for the content box by subtracting padding and borders.
    pub fn content_constraints(&self, constraints: BoxConstraints) -> BoxConstraints {
        let h_deduction = self.padding_x() + self.border_x();
        // Note: Vertical deduction usually isn't applied to constraints in standard flow unless height is fixed,
        // but for safety we typically pass infinite height for children unless constrained.
        // Here we just handle width primarily for block flow.

        if constraints.has_bounded_width() {
            let max_w = (constraints.max_width - h_deduction).max(0.0);
            BoxConstraints {
                min_width: 0.0,
                max_width: max_w,
                min_height: 0.0,
                max_height: f32::INFINITY,
            }
        } else {
            BoxConstraints {
                min_width: 0.0,
                max_width: f32::INFINITY,
                min_height: 0.0,
                max_height: f32::INFINITY,
            }
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
        // Reset non-inherited box model properties
        computed.box_model = BoxModel::default();
        computed.border = BorderModel::default();
        computed.misc.background_color = None;
        computed.flex = FlexModel {
            shrink: 1.0, // Default shrink is 1.0
            ..Default::default()
        };
        computed.table = TableModel::default();

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
        text: TextModel {
            font_family: merged
                .font_family
                .map(Arc::new)
                .unwrap_or_else(|| parent_style.text.font_family.clone()),
            font_size: merged.font_size.unwrap_or(parent_style.text.font_size),
            font_weight: merged
                .font_weight
                .unwrap_or_else(|| parent_style.text.font_weight.clone()),
            font_style: merged
                .font_style
                .unwrap_or_else(|| parent_style.text.font_style.clone()),
            line_height: merged.line_height.unwrap_or_else(|| {
                merged
                    .font_size
                    .map(|fs| fs * 1.2)
                    .unwrap_or(parent_style.text.line_height)
            }),
            text_align: merged
                .text_align
                .unwrap_or_else(|| parent_style.text.text_align.clone()),
            text_decoration: merged
                .text_decoration
                .unwrap_or_else(|| parent_style.text.text_decoration.clone()),
            color: merged.color.unwrap_or_else(|| parent_style.text.color.clone()),
        },
        misc: MiscModel {
            widows: merged.widows.unwrap_or(parent_style.misc.widows),
            orphans: merged.orphans.unwrap_or(parent_style.misc.orphans),
            background_color: merged.background_color,
        },
        list: ListModel {
            style_type: merged
                .list_style_type
                .unwrap_or_else(|| parent_style.list.style_type.clone()),
            style_position: merged
                .list_style_position
                .unwrap_or_else(|| parent_style.list.style_position.clone()),
        },
        table: TableModel {
            border_spacing: merged.border_spacing.unwrap_or(parent_style.table.border_spacing),
        },
        // Non-inherited properties
        box_model: BoxModel {
            margin: merged.margin.unwrap_or_default(),
            padding: merged.padding.unwrap_or_default(),
            width: merged.width,
            height: merged.height,
            min_height: Dimension::Auto,
        },
        border: BorderModel {
            top: merged.border_top.or_else(|| merged.border.clone()),
            right: merged.border_right.or_else(|| merged.border.clone()),
            bottom: merged.border_bottom.or_else(|| merged.border.clone()),
            left: merged.border_left.or_else(|| merged.border.clone()),
        },
        flex: FlexModel {
            direction: merged.flex_direction.unwrap_or_default(),
            wrap: merged.flex_wrap.unwrap_or_default(),
            justify_content: merged.justify_content.unwrap_or_default(),
            align_items: merged.align_items.unwrap_or_default(),
            align_content: JustifyContent::FlexStart,
            order: merged.order.unwrap_or_default(),
            grow: merged.flex_grow.unwrap_or_default(),
            shrink: merged.flex_shrink.unwrap_or(1.0),
            basis: merged.flex_basis.unwrap_or_default(),
            align_self: merged.align_self.unwrap_or_default(),
        },
    };

    Arc::new(computed)
}

/// Returns the default style for the document root.
pub fn get_default_style() -> Arc<ComputedStyle> {
    Arc::new(ComputedStyle::default())
}

/// Merges properties from `to_apply` into `base`.
fn merge_element_styles(base: &mut ElementStyle, to_apply: &ElementStyle) {
    if to_apply.font_family.is_some() { base.font_family = to_apply.font_family.clone(); }
    if to_apply.font_size.is_some() { base.font_size = to_apply.font_size; }
    if to_apply.font_weight.is_some() { base.font_weight = to_apply.font_weight.clone(); }
    if to_apply.font_style.is_some() { base.font_style = to_apply.font_style.clone(); }
    if to_apply.line_height.is_some() { base.line_height = to_apply.line_height; }
    if to_apply.text_align.is_some() { base.text_align = to_apply.text_align.clone(); }
    if to_apply.color.is_some() { base.color = to_apply.color.clone(); }
    if to_apply.text_decoration.is_some() { base.text_decoration = to_apply.text_decoration.clone(); }
    if to_apply.widows.is_some() { base.widows = to_apply.widows; }
    if to_apply.orphans.is_some() { base.orphans = to_apply.orphans; }
    if to_apply.background_color.is_some() { base.background_color = to_apply.background_color.clone(); }
    if to_apply.border.is_some() { base.border = to_apply.border.clone(); }
    if to_apply.border_top.is_some() { base.border_top = to_apply.border_top.clone(); }
    if to_apply.border_right.is_some() { base.border_right = to_apply.border_right.clone(); }
    if to_apply.border_bottom.is_some() { base.border_bottom = to_apply.border_bottom.clone(); }
    if to_apply.border_left.is_some() { base.border_left = to_apply.border_left.clone(); }
    if to_apply.margin.is_some() { base.margin = to_apply.margin.clone(); }
    if to_apply.padding.is_some() { base.padding = to_apply.padding.clone(); }
    if to_apply.width.is_some() { base.width = to_apply.width.clone(); }
    if to_apply.height.is_some() { base.height = to_apply.height.clone(); }
    if to_apply.list_style_type.is_some() { base.list_style_type = to_apply.list_style_type.clone(); }
    if to_apply.list_style_position.is_some() { base.list_style_position = to_apply.list_style_position.clone(); }
    if to_apply.list_style_image.is_some() { base.list_style_image = to_apply.list_style_image.clone(); }
    if to_apply.border_spacing.is_some() { base.border_spacing = to_apply.border_spacing; }
    if to_apply.flex_direction.is_some() { base.flex_direction = to_apply.flex_direction.clone(); }
    if to_apply.flex_wrap.is_some() { base.flex_wrap = to_apply.flex_wrap.clone(); }
    if to_apply.justify_content.is_some() { base.justify_content = to_apply.justify_content.clone(); }
    if to_apply.align_items.is_some() { base.align_items = to_apply.align_items.clone(); }
    if to_apply.order.is_some() { base.order = to_apply.order; }
    if to_apply.flex_grow.is_some() { base.flex_grow = to_apply.flex_grow; }
    if to_apply.flex_shrink.is_some() { base.flex_shrink = to_apply.flex_shrink; }
    if to_apply.flex_basis.is_some() { base.flex_basis = to_apply.flex_basis.clone(); }
    if to_apply.align_self.is_some() { base.align_self = to_apply.align_self.clone(); }
}