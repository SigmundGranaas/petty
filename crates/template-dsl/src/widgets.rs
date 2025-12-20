use crate::builders::*;
use crate::node::TemplateBuilder;
use crate::style::StyledWidget;
use petty_style::font::FontWeight;
use petty_style::text::TextAlign;
use petty_types::color::Color;

// Semantic Text Elements
pub fn h1(text: &str) -> Paragraph {
    Paragraph::new(text)
        .font_size(28.0)
        .font_weight(FontWeight::Bold)
        .color(Color::gray(34))
}

pub fn h2(text: &str) -> Paragraph {
    Paragraph::new(text)
        .font_size(18.0)
        .font_weight(FontWeight::Bold)
}

pub fn h3(text: &str) -> Paragraph {
    Paragraph::new(text)
        .font_size(9.0)
        .font_weight(FontWeight::Bold)
        .color(Color::gray(136))
}

pub fn p(text: &str) -> Paragraph {
    Paragraph::new(text)
}

pub fn subtitle(text: &str) -> Paragraph {
    Paragraph::new(text).font_size(10.0).color(Color::gray(102))
}

// A simple container with right-aligned content
pub fn align_right(child: impl TemplateBuilder + 'static) -> Block {
    Block::new().text_align(TextAlign::Right).child(child)
}
