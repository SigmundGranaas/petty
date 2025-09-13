// src/layout/text.rs

//! Text measurement and paragraph layout with line breaking.

use super::elements::{LayoutElement, PositionedElement, TextElement};
use super::style::ComputedStyle;
use super::{InlineNode, LayoutEngine};
use crate::stylesheet::{Color, TextAlign};

/// A temporary representation of a text fragment with its style.
#[derive(Debug, Clone)]
struct StyledFragment {
    text: String,
    style: ComputedStyle,
}

/// Converts a flat list of styled fragments back into a simplified Vec of InlineNodes.
/// This is used to reconstruct the "remaining" content when a paragraph is split.
fn convert_fragments_to_inlines(fragments: Vec<StyledFragment>) -> Vec<InlineNode> {
    if fragments.is_empty() {
        return vec![];
    }
    // TODO: A more sophisticated implementation would re-create styled spans.
    // For now, concatenate all remaining text into a single Text node.
    let combined_text = fragments.into_iter().map(|f| f.text).collect::<String>();
    if !combined_text.is_empty() {
        vec![InlineNode::Text(combined_text)]
    } else {
        vec![]
    }
}


/// Lays out a paragraph with line breaking, respecting a maximum height constraint.
pub fn layout_paragraph(
    engine: &LayoutEngine,
    inlines: &[InlineNode],
    style: &ComputedStyle,
    available_width: f32,
    max_height: f32,
) -> (Vec<PositionedElement>, f32, Option<Vec<InlineNode>>) {
    let mut elements = Vec::new();
    let fragments = get_styled_fragments(engine, inlines, style);

    let box_width = available_width;
    let mut current_y = 0.0;
    let mut line_fragments = Vec::new();
    let mut current_line_width = 0.0;

    let mut all_fragments = fragments.into_iter().collect::<Vec<_>>();
    let mut frag_idx = 0;

    while frag_idx < all_fragments.len() {
        let frag = &all_fragments[frag_idx];

        if frag.text == "\n" {
            if current_y + style.line_height > max_height && !line_fragments.is_empty() {
                let pending = all_fragments.split_off(frag_idx);
                return (elements, current_y, Some(convert_fragments_to_inlines(pending)));
            }
            commit_line(engine, &mut elements, style, std::mem::take(&mut line_fragments), box_width, current_y);
            current_y += style.line_height;
            current_line_width = 0.0;
            frag_idx += 1;
            continue;
        }

        let words = frag.text.split_inclusive(' ').collect::<Vec<_>>();
        let mut word_idx = 0;

        while word_idx < words.len() {
            let word_str = words[word_idx];
            if word_str.trim().is_empty() && line_fragments.is_empty() {
                word_idx += 1;
                continue;
            }
            let word_width = engine.measure_text_width(word_str, &frag.style);

            if !line_fragments.is_empty() && (current_line_width + word_width) > box_width {
                if current_y + style.line_height > max_height {
                    let remaining_text_in_frag = words[word_idx..].join("");
                    let mut pending_fragments = vec![StyledFragment { text: remaining_text_in_frag, style: frag.style.clone() }];
                    if frag_idx + 1 < all_fragments.len() {
                        pending_fragments.extend(all_fragments[(frag_idx + 1)..].iter().cloned());
                    }
                    return (elements, current_y, Some(convert_fragments_to_inlines(pending_fragments)));
                }
                commit_line(engine, &mut elements, style, std::mem::take(&mut line_fragments), box_width, current_y);
                current_y += style.line_height;
                current_line_width = 0.0;
            }

            line_fragments.push(StyledFragment { text: word_str.to_string(), style: frag.style.clone() });
            current_line_width += word_width;
            word_idx += 1;
        }
        frag_idx += 1;
    }

    if !line_fragments.is_empty() {
        if current_y + style.line_height > max_height && !elements.is_empty() {
            return (elements, current_y, Some(convert_fragments_to_inlines(line_fragments)));
        }
        commit_line(engine, &mut elements, style, line_fragments, box_width, current_y);
        current_y += style.line_height;
    }

    (elements, current_y, None)
}

/// Helper to position and generate elements for a single line of text.
fn commit_line(
    engine: &LayoutEngine,
    elements: &mut Vec<PositionedElement>,
    parent_style: &ComputedStyle,
    line_fragments: Vec<StyledFragment>,
    box_width: f32,
    y: f32,
) {
    if line_fragments.is_empty() {
        return;
    }

    let line_style = line_fragments.first().map(|f| f.style.clone()).unwrap_or_else(|| parent_style.clone());
    let combined_text_for_line: String = line_fragments.into_iter().map(|f| f.text).collect();
    let trimmed_line_text = combined_text_for_line.trim_end();
    let final_line_width = engine.measure_text_width(trimmed_line_text, &line_style);

    let element_width = final_line_width.min(box_width);

    let start_x = match parent_style.text_align {
        TextAlign::Left => 0.0,
        TextAlign::Center => (box_width - element_width) / 2.0,
        TextAlign::Right => box_width - element_width,
        TextAlign::Justify => 0.0,
    };

    elements.push(PositionedElement {
        x: start_x,
        y,
        width: element_width,
        height: line_style.line_height,
        element: LayoutElement::Text(TextElement {
            content: trimmed_line_text.to_string(),
        }),
        style: line_style,
    });
}


/// Traverses inline nodes to produce a flat list of text fragments with their computed styles.
fn get_styled_fragments(
    engine: &LayoutEngine,
    inlines: &[InlineNode],
    parent_style: &ComputedStyle,
) -> Vec<StyledFragment> {
    let mut fragments = Vec::new();
    for inline in inlines {
        match inline {
            InlineNode::Text(text) => fragments.push(StyledFragment {
                text: text.clone(),
                style: parent_style.clone(),
            }),
            InlineNode::StyledSpan {
                style_name,
                children,
            } => {
                let style = engine.compute_style(style_name.as_deref(), parent_style);
                fragments.extend(get_styled_fragments(engine, children, &style));
            }
            InlineNode::Hyperlink {
                children,
                style_name,
                ..
            } => {
                let mut style = engine.compute_style(style_name.as_deref(), parent_style);
                style.color = Color {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 1.0,
                }; // Simple link styling
                fragments.extend(get_styled_fragments(engine, children, &style));
            }
            InlineNode::LineBreak => {
                fragments.push(StyledFragment {
                    text: "\n".to_string(),
                    style: parent_style.clone(),
                });
            }
            _ => {}
        }
    }
    fragments
}

/// Measures the width of a text string based on its style.
/// This is a simple approximation. A real implementation would use a font metrics library.
pub fn measure_text_width(
    _engine: &LayoutEngine,
    text: &str,
    style: &ComputedStyle,
) -> f32 {
    let char_width = style.font_size * 0.6; // Approximation
    text.chars().count() as f32 * char_width
}


#[cfg(test)]
mod tests {
    use crate::layout::style;
    use super::*;
    use crate::stylesheet::Stylesheet;

    // A mock engine for testing purposes
    fn create_test_engine() -> LayoutEngine {
        LayoutEngine::new(Stylesheet::default())
    }

    #[test]
    fn test_simple_line_break() {
        let engine = create_test_engine();
        let style = style::get_default_style(); // font_size: 12.0, line_height: 14.4
        let inlines = vec![
            InlineNode::Text("This is a long line of text that should wrap.".to_string())
        ];

        // "This is a long line of text that should wrap." has 45 characters.
        // measure_text_width: 45 * 12.0 * 0.6 = 324.0.
        // We set available_width to 200.0, so it *must* wrap.
        let (elements, height, remainder) = layout_paragraph(&engine, &inlines, &style, 200.0, 1000.0);

        assert!(remainder.is_none());
        assert_eq!(elements.len(), 2, "Text should have wrapped into exactly two lines");
        let first_y = elements[0].y;
        let second_y = elements[1].y;
        assert!(second_y > first_y, "Second line's Y position should be greater than the first");
        assert!((second_y - first_y).abs() - style.line_height < 0.01, "Second line should be one line_height below the first");

        assert!((height - 2.0 * 14.4).abs() < 0.01, "Paragraph height should match two line heights");
    }

    #[test]
    fn test_explicit_line_break() {
        let engine = create_test_engine();
        let style = style::get_default_style();
        let inlines = vec![
            InlineNode::Text("Line 1".to_string()),
            InlineNode::LineBreak,
            InlineNode::Text("Line 2".to_string()),
        ];

        let (elements, height, remainder) = layout_paragraph(&engine, &inlines, &style, 500.0, 1000.0);

        assert!(remainder.is_none());
        assert_eq!(elements.len(), 2, "There should be exactly two PositionedElements for two lines");
        assert_eq!(elements[0].y, 0.0);
        assert!((elements[1].y - 14.4).abs() < 0.01, "Second line should be one line_height below the first");
        assert!((height - 2.0 * 14.4).abs() < 0.01, "Paragraph height should match two line heights");
    }

    #[test]
    fn test_text_alignment() {
        let engine = create_test_engine();
        let mut style = style::get_default_style();
        style.text_align = TextAlign::Center;
        let inlines = vec![InlineNode::Text("Centered".to_string())];

        let available_width = 500.0;
        let (elements, _, _) = layout_paragraph(&engine, &inlines, &style, available_width, 1000.0);

        let text_width = measure_text_width(&engine, "Centered", &style);
        let expected_x = (available_width - text_width) / 2.0;

        assert!(!elements.is_empty());
        assert!((elements[0].x - expected_x).abs() < 0.01);
    }
}