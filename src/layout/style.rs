// src/layout/style.rs

//! Style computation and management.

use crate::stylesheet::{
    Color, Dimension, FontStyle, FontWeight, Margins, PageSize, Stylesheet, TextAlign,
};

/// A fully resolved style with no optional values, ready for layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: f32,
    pub text_align: TextAlign,
    pub color: Color,
    pub margin: Margins,
    pub padding: Margins,
    pub width: Option<Dimension>,
    pub height: Option<f32>,
    pub background_color: Option<Color>,
    pub border: Option<crate::stylesheet::Border>,
}

/// Computes the style for a node by inheriting from its parent and applying
/// any rules from the stylesheet that match its `style_name`.
pub fn compute_style(
    stylesheet: &Stylesheet,
    style_name: Option<&str>,
    parent_style: &ComputedStyle,
) -> ComputedStyle {
    let inherited_style = parent_style.clone();
    apply_style_rules(stylesheet, inherited_style, style_name)
}

/// Returns the default style for the document root.
pub fn get_default_style() -> ComputedStyle {
    ComputedStyle {
        font_family: "Helvetica".to_string(),
        font_size: 12.0,
        font_weight: FontWeight::Regular,
        font_style: FontStyle::Normal,
        line_height: 14.4, // 12.0 * 1.2
        text_align: TextAlign::Left,
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
        border: None,
    }
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

/// Applies style rules from a stylesheet to a computed style.
fn apply_style_rules(
    stylesheet: &Stylesheet,
    mut computed: ComputedStyle,
    style_name: Option<&str>,
) -> ComputedStyle {
    if let Some(name) = style_name {
        if let Some(style_def) = stylesheet.styles.get(name) {
            if let Some(ff) = &style_def.font_family {
                computed.font_family = ff.clone();
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
                if let Dimension::Pt(h_pt) = h {
                    computed.height = Some(*h_pt)
                }
            }
            if let Some(bg) = &style_def.background_color {
                computed.background_color = Some(bg.clone());
            }
            if let Some(b) = &style_def.border {
                computed.border = Some(b.clone());
            }
        }
    }
    computed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stylesheet::ElementStyle; // Corrected import for ElementStyle
    use std::collections::HashMap;

    fn create_test_stylesheet() -> Stylesheet {
        let mut styles = HashMap::new();
        styles.insert(
            "override".to_string(),
            ElementStyle { // Use ElementStyle here
                font_size: Some(20.0),
                color: Some(Color { r: 255, g: 0, b: 0, a: 1.0 }),
                ..Default::default()
            },
        );
        Stylesheet {
            styles,
            page: Default::default(),
            templates: HashMap::new(), // Added missing field
            page_sequences: HashMap::new(), // Added missing field
        }
    }

    #[test]
    fn test_default_style() {
        let default = get_default_style();
        assert_eq!(default.font_size, 12.0);
        assert_eq!(default.line_height, 14.4);
    }

    #[test]
    fn test_style_application() {
        let stylesheet = create_test_stylesheet();
        let parent_style = get_default_style();
        let computed = compute_style(&stylesheet, Some("override"), &parent_style);

        assert_eq!(computed.font_size, 20.0);
        assert_eq!(computed.line_height, 24.0); // 20.0 * 1.2
        assert_eq!(computed.color.r, 255);
        assert_eq!(computed.font_family, "Helvetica"); // Inherited
    }

    #[test]
    fn test_no_style_name() {
        let stylesheet = create_test_stylesheet();
        let parent_style = get_default_style();
        let computed = compute_style(&stylesheet, None, &parent_style);

        // Should be identical to the parent style
        assert_eq!(computed, parent_style);
    }
}