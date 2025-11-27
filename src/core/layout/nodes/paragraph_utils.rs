use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutContext, LayoutElement, PositionedElement, TextElement};
use std::sync::Arc;

pub fn flush_group(
    ctx: &mut LayoutContext,
    glyphs: &[&cosmic_text::LayoutGlyph],
    metadata: usize,
    y: f32,
    height: f32,
    style: &Arc<ComputedStyle>,
    links: &[String],
    text: &str,
) {
    if glyphs.is_empty() { return; }

    let start_x = glyphs.first().unwrap().x;
    let end_x = glyphs.last().unwrap().x + glyphs.last().unwrap().w;
    let width = end_x - start_x;

    let is_image = (metadata & (1 << 31)) != 0;
    if is_image {
        return;
    }

    let href = if metadata > 0 && metadata <= links.len() {
        Some(links[metadata - 1].clone())
    } else {
        None
    };

    let start_byte = glyphs.first().unwrap().start;
    let end_byte = glyphs.last().unwrap().end;
    let content = text[start_byte..end_byte].to_string();

    let element = PositionedElement {
        x: start_x,
        y,
        width,
        height,
        element: LayoutElement::Text(TextElement {
            content,
            href,
            text_decoration: style.text.text_decoration.clone(),
        }),
        style: style.clone(),
    };
    ctx.push_element(element);
}