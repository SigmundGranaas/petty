use crate::layout::style::ComputedStyle;
use crate::stylesheet::{
    Color, Dimension, ElementStyle, FontStyle, FontWeight, Margins, PageLayout, PageSize,
    Stylesheet, TextAlign,
};
use std::collections::HashMap;

/// A helper struct that centralizes style computation and text wrapping logic.
pub struct LayoutEngine {
    pub page_layout: PageLayout,
    pub styles: HashMap<String, ElementStyle>,
}

impl LayoutEngine {
    pub fn new(stylesheet: &Stylesheet) -> Self {
        LayoutEngine {
            page_layout: stylesheet.page.clone(),
            styles: stylesheet.styles.clone(),
        }
    }

    pub fn get_page_dimensions(&self) -> (f32, f32) {
        match self.page_layout.size {
            PageSize::A4 => (595.0, 842.0),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Custom { width, height } => (width, height),
        }
    }

    pub fn wrap_text(&self, text: &str, style: &ComputedStyle, max_width: f32) -> Vec<String> {
        if max_width <= 0.0 {
            return text.lines().map(|s| s.to_string()).collect();
        }
        let mut lines = Vec::new();
        for paragraph in text.lines() {
            if paragraph.trim().is_empty() {
                lines.push("".to_string());
                continue;
            }
            let words = paragraph.split_whitespace();
            let mut current_line = String::new();
            // This is a rough approximation. A real implementation would use a font library
            // to measure text width accurately.
            let char_width = style.font_size * 0.6;

            for word in words {
                let test_line = if current_line.is_empty() {
                    word.to_string()
                } else {
                    format!("{} {}", current_line, word)
                };

                let line_width = test_line.len() as f32 * char_width;

                if line_width > max_width && !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::from(word);
                } else {
                    current_line = test_line;
                }
            }

            if !current_line.is_empty() {
                lines.push(current_line);
            }
        }
        lines
    }

    pub fn compute_style_from_default(&self, style_name: Option<&str>) -> ComputedStyle {
        let default_style = ComputedStyle {
            font_family: "Helvetica".to_string(),
            font_size: 12.0,
            font_weight: FontWeight::Regular,
            font_style: FontStyle::Normal,
            line_height: 14.4,
            text_align: TextAlign::Left,
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 1.0,
            },
            margin: Margins {
                top: 0.0,
                right: 0.0,
                bottom: 10.0,
                left: 0.0,
            },
            padding: Margins {
                top: 2.0,
                right: 2.0,
                bottom: 2.0,
                left: 2.0,
            },
            width: None,
            height: None,
            background_color: None,
            border: None,
        };
        self.apply_style_rules(default_style, style_name)
    }

    pub fn compute_style_from_parent(
        &self,
        style_name: Option<&str>,
        parent_style: &ComputedStyle,
    ) -> ComputedStyle {
        let inherited_style = parent_style.clone();
        self.apply_style_rules(inherited_style, style_name)
    }

    fn apply_style_rules(
        &self,
        mut computed: ComputedStyle,
        style_name: Option<&str>,
    ) -> ComputedStyle {
        if let Some(name) = style_name {
            if let Some(style_def) = self.styles.get(name) {
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
                if let Some(Dimension::Pt(w)) = style_def.width {
                    computed.width = Some(w);
                }
                if let Some(Dimension::Pt(h)) = style_def.height {
                    computed.height = Some(h);
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
}