use crate::stylesheet::{Border, Color, FontStyle, FontWeight, Margins, TextAlign};

#[derive(Clone, Debug, Default)]
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
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub background_color: Option<Color>,
    pub border: Option<Border>,
}