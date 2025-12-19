use crate::core::layout::style::{BorderModel, ComputedStyle};
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::flex::{AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};
use taffy::style::{LengthPercentage, LengthPercentageAuto};

pub fn computed_style_to_taffy(style: &ComputedStyle) -> taffy::style::Style {
    taffy::style::Style {
        display: taffy::style::Display::Flex,
        box_sizing: taffy::style::BoxSizing::BorderBox,
        size: taffy::geometry::Size {
            width: to_taffy_dimension(&style.box_model.width),
            height: to_taffy_dimension(&style.box_model.height),
        },
        min_size: taffy::geometry::Size {
            width: taffy::style::Dimension::auto(),
            height: to_taffy_dimension(&Some(style.box_model.min_height.clone())),
        },
        margin: to_taffy_margin(&style.box_model.margin),
        padding: to_taffy_padding(&style.box_model.padding),
        border: to_taffy_border(&style.border),
        align_items: to_taffy_align_items(style.flex.align_items.clone()),
        align_self: to_taffy_align_self(style.flex.align_self.clone()),
        justify_content: to_taffy_justify_content(style.flex.justify_content.clone()),
        flex_direction: to_taffy_flex_direction(style.flex.direction.clone()),
        flex_wrap: to_taffy_flex_wrap(style.flex.wrap.clone()),
        flex_grow: style.flex.grow,
        flex_shrink: style.flex.shrink,
        flex_basis: to_taffy_dimension(&Some(style.flex.basis.clone())),
        ..Default::default()
    }
}

pub fn to_taffy_dimension(d: &Option<Dimension>) -> taffy::style::Dimension {
    match d {
        Some(Dimension::Pt(v)) => taffy::style::Dimension::length(*v),
        Some(Dimension::Percent(v)) => taffy::style::Dimension::percent(v / 100.0),
        Some(Dimension::Auto) | None => taffy::style::Dimension::auto(),
    }
}

pub fn to_taffy_margin(m: &Margins) -> taffy::geometry::Rect<LengthPercentageAuto> {
    taffy::geometry::Rect {
        left: LengthPercentageAuto::length(m.left),
        right: LengthPercentageAuto::length(m.right),
        top: LengthPercentageAuto::length(m.top),
        bottom: LengthPercentageAuto::length(m.bottom),
    }
}

pub fn to_taffy_padding(m: &Margins) -> taffy::geometry::Rect<LengthPercentage> {
    taffy::geometry::Rect {
        left: LengthPercentage::length(m.left),
        right: LengthPercentage::length(m.right),
        top: LengthPercentage::length(m.top),
        bottom: LengthPercentage::length(m.bottom),
    }
}

pub fn to_taffy_border(b: &BorderModel) -> taffy::geometry::Rect<LengthPercentage> {
    taffy::geometry::Rect {
        left: LengthPercentage::length(b.left.as_ref().map_or(0.0, |x| x.width)),
        right: LengthPercentage::length(b.right.as_ref().map_or(0.0, |x| x.width)),
        top: LengthPercentage::length(b.top.as_ref().map_or(0.0, |x| x.width)),
        bottom: LengthPercentage::length(b.bottom.as_ref().map_or(0.0, |x| x.width)),
    }
}

pub fn to_taffy_align_items(a: AlignItems) -> Option<taffy::style::AlignItems> {
    match a {
        AlignItems::Stretch => Some(taffy::style::AlignItems::Stretch),
        AlignItems::FlexStart => Some(taffy::style::AlignItems::FlexStart),
        AlignItems::FlexEnd => Some(taffy::style::AlignItems::FlexEnd),
        AlignItems::Center => Some(taffy::style::AlignItems::Center),
        AlignItems::Baseline => Some(taffy::style::AlignItems::Baseline),
    }
}

pub fn to_taffy_align_self(a: AlignSelf) -> Option<taffy::style::AlignSelf> {
    match a {
        AlignSelf::Auto => None,
        AlignSelf::Stretch => Some(taffy::style::AlignSelf::Stretch),
        AlignSelf::FlexStart => Some(taffy::style::AlignSelf::FlexStart),
        AlignSelf::FlexEnd => Some(taffy::style::AlignSelf::FlexEnd),
        AlignSelf::Center => Some(taffy::style::AlignSelf::Center),
        AlignSelf::Baseline => Some(taffy::style::AlignSelf::Baseline),
    }
}

pub fn to_taffy_justify_content(j: JustifyContent) -> Option<taffy::style::JustifyContent> {
    match j {
        JustifyContent::FlexStart => Some(taffy::style::JustifyContent::FlexStart),
        JustifyContent::FlexEnd => Some(taffy::style::JustifyContent::FlexEnd),
        JustifyContent::Center => Some(taffy::style::JustifyContent::Center),
        JustifyContent::SpaceBetween => Some(taffy::style::JustifyContent::SpaceBetween),
        JustifyContent::SpaceAround => Some(taffy::style::JustifyContent::SpaceAround),
        JustifyContent::SpaceEvenly => Some(taffy::style::JustifyContent::SpaceEvenly),
    }
}

pub fn to_taffy_flex_direction(f: FlexDirection) -> taffy::style::FlexDirection {
    match f {
        FlexDirection::Row => taffy::style::FlexDirection::Row,
        FlexDirection::RowReverse => taffy::style::FlexDirection::RowReverse,
        FlexDirection::Column => taffy::style::FlexDirection::Column,
        FlexDirection::ColumnReverse => taffy::style::FlexDirection::ColumnReverse,
    }
}

pub fn to_taffy_flex_wrap(f: FlexWrap) -> taffy::style::FlexWrap {
    match f {
        FlexWrap::NoWrap => taffy::style::FlexWrap::NoWrap,
        FlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
        FlexWrap::WrapReverse => taffy::style::FlexWrap::WrapReverse,
    }
}