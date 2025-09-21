// src/layout/text.rs

//! Text measurement and paragraph layout with line breaking.

use super::elements::{ImageElement, LayoutElement, PositionedElement, TextElement};
use super::style::ComputedStyle;
use super::{InlineNode, LayoutEngine, WorkItem};
use crate::idf::{IRNode, SharedData};
use crate::stylesheet::{Color, Dimension, ElementStyle, TextAlign};
use std::sync::Arc;

/// Represents a piece of content that can be placed on a line (text or image).
#[derive(Debug, Clone)]
enum LineContent {
    Text(String),
    Image {
        src: String,
        data: Option<SharedData>,
        width: f32,
        height: f32,
    },
}

/// An intermediate representation of an inline item with its style, width, and link URL.
#[derive(Debug, Clone)]
struct LineItem {
    content: LineContent,
    width: f32,
    style: Arc<ComputedStyle>,
    href: Option<String>,
}

/// Converts a flat list of styled items back into a simplified Vec of InlineNodes.
/// This is used to reconstruct the "remaining" content when a paragraph is split.
fn convert_items_to_inlines(items: Vec<LineItem>) -> Vec<InlineNode> {
    if items.is_empty() {
        return vec![];
    }
    // TODO: A more sophisticated implementation would re-create styled spans, images, etc.
    // For now, concatenate all remaining text into a single Text node.
    let combined_text = items
        .into_iter()
        .filter_map(|item| match item.content {
            LineContent::Text(text) => Some(text),
            _ => None,
        })
        .collect::<String>();

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
    style: &Arc<ComputedStyle>,
    available_width: f32,
    max_height: f32,
) -> (Vec<PositionedElement>, f32, Option<Vec<InlineNode>>) {
    let line_items = flatten_inlines(engine, inlines, style, None);
    // PERF: Pre-allocate vectors with a reasonable guess to avoid reallocations.
    let mut elements = Vec::with_capacity(line_items.len() / 5); // Guess: 5 items form a single rendered line
    let mut line_buffer = Vec::with_capacity(20); // Guess: 20 items (words, images) per line

    let box_width = available_width;
    let mut current_y = 0.0;
    let mut current_line_width = 0.0;

    let mut all_items = line_items;
    let mut item_idx = 0;

    while item_idx < all_items.len() {
        let item = &all_items[item_idx];

        // Handle explicit line breaks from `<br/>` tags
        if let LineContent::Text(text) = &item.content {
            if text == "\n" {
                if current_y + style.line_height > max_height && !elements.is_empty() {
                    let pending = all_items.split_off(item_idx);
                    return (elements, current_y, Some(convert_items_to_inlines(pending)));
                }
                current_y += commit_line(
                    &mut elements,
                    style,
                    std::mem::take(&mut line_buffer),
                    box_width,
                    current_y,
                );
                current_line_width = 0.0;
                item_idx += 1;
                continue;
            }
        }

        // Word-wrapping logic
        match &item.content {
            LineContent::Text(text) => {
                let words = text.split_inclusive(' ').collect::<Vec<_>>();
                let mut word_idx = 0;
                while word_idx < words.len() {
                    let word_str = words[word_idx];
                    if word_str.trim().is_empty() && line_buffer.is_empty() {
                        word_idx += 1;
                        continue;
                    }
                    let word_width = engine.measure_text_width(word_str, &item.style);

                    if !line_buffer.is_empty() && (current_line_width + word_width) > box_width {
                        if current_y + style.line_height > max_height {
                            let remaining_text_in_item = words[word_idx..].join("");
                            // PERF: Pre-allocate pending_items vector.
                            let mut pending_items = Vec::with_capacity(1 + all_items.len().saturating_sub(item_idx + 1));
                            pending_items.push(LineItem {
                                content: LineContent::Text(remaining_text_in_item),
                                width: 0.0, // Recalculated later
                                style: item.style.clone(),
                                href: item.href.clone(),
                            });
                            if item_idx + 1 < all_items.len() {
                                pending_items.extend(all_items[(item_idx + 1)..].iter().cloned());
                            }
                            return (
                                elements,
                                current_y,
                                Some(convert_items_to_inlines(pending_items)),
                            );
                        }
                        current_y += commit_line(
                            &mut elements,
                            style,
                            std::mem::take(&mut line_buffer),
                            box_width,
                            current_y,
                        );
                        current_line_width = 0.0;
                    }
                    line_buffer.push(LineItem {
                        content: LineContent::Text(word_str.to_string()),
                        width: word_width,
                        style: item.style.clone(),
                        href: item.href.clone(),
                    });
                    current_line_width += word_width;
                    word_idx += 1;
                }
            }
            LineContent::Image { .. } => {
                // Treat image as a single, unbreakable word
                if !line_buffer.is_empty() && (current_line_width + item.width) > box_width {
                    if current_y + style.line_height > max_height {
                        let pending_items = all_items.split_off(item_idx);
                        return (
                            elements,
                            current_y,
                            Some(convert_items_to_inlines(pending_items)),
                        );
                    }
                    current_y += commit_line(
                        &mut elements,
                        style,
                        std::mem::take(&mut line_buffer),
                        box_width,
                        current_y,
                    );
                    current_line_width = 0.0;
                }
                line_buffer.push(item.clone());
                current_line_width += item.width;
            }
        }
        item_idx += 1;
    }

    if !line_buffer.is_empty() {
        if current_y + style.line_height > max_height && !elements.is_empty() {
            return (elements, current_y, Some(convert_items_to_inlines(line_buffer)));
        }
        current_y += commit_line(&mut elements, style, line_buffer, box_width, current_y);
    }

    (elements, current_y, None)
}

/// A cheap, measurement-only version of layout_paragraph. It calculates the final
/// height of the paragraph without allocating any `PositionedElement`s.
pub fn measure_paragraph_height(
    engine: &LayoutEngine,
    inlines: &[InlineNode],
    style: &Arc<ComputedStyle>,
    available_width: f32,
) -> f32 {
    let line_items = flatten_inlines(engine, inlines, style, None);
    if line_items.is_empty() {
        return 0.0;
    }

    let box_width = available_width;
    let mut line_count = 1;
    let mut current_line_width = 0.0;

    for item in &line_items {
        // Handle explicit line breaks
        if let LineContent::Text(text) = &item.content {
            if text == "\n" {
                line_count += 1;
                current_line_width = 0.0;
                continue;
            }
        }

        // Word-wrapping logic
        if let LineContent::Text(text) = &item.content {
            let words = text.split_inclusive(' ').collect::<Vec<_>>();
            for word_str in words {
                if word_str.trim().is_empty() && current_line_width == 0.0 {
                    continue; // Skip leading whitespace on a new line
                }
                let word_width = engine.measure_text_width(word_str, &item.style);

                if current_line_width > 0.0 && (current_line_width + word_width) > box_width {
                    line_count += 1;
                    current_line_width = 0.0;
                }
                current_line_width += word_width;
            }
        } else {
            // Treat image as a single, unbreakable word
            if current_line_width > 0.0 && (current_line_width + item.width) > box_width {
                line_count += 1;
                current_line_width = 0.0;
            }
            current_line_width += item.width;
        }
    }

    line_count as f32 * style.line_height
}

/// Helper to position and generate elements for a single line of content.
/// Returns the height consumed by the line.
fn commit_line(
    elements: &mut Vec<PositionedElement>,
    parent_style: &Arc<ComputedStyle>,
    line_items: Vec<LineItem>,
    box_width: f32,
    start_y: f32,
) -> f32 {
    if line_items.is_empty() {
        return parent_style.line_height;
    }

    let total_content_width: f32 = line_items.iter().map(|item| item.width).sum();
    let mut current_x = match parent_style.text_align {
        TextAlign::Left => 0.0,
        TextAlign::Center => (box_width - total_content_width) / 2.0,
        TextAlign::Right => box_width - total_content_width,
        TextAlign::Justify => 0.0, // Justify not implemented
    };

    let mut items_iter = line_items.into_iter().peekable();

    while let Some(item) = items_iter.next() {
        let item_width = item.width;
        match item.content {
            LineContent::Text(text) => {
                let mut text_run = text;
                let mut run_width = item.width;
                let style = item.style;
                let href = item.href;

                // Peek ahead to see if we can merge consecutive text items
                while let Some(next_item) = items_iter.peek() {
                    if let LineContent::Text(next_text) = &next_item.content {
                        if Arc::ptr_eq(&next_item.style, &style) && next_item.href == href {
                            // It's a match, so consume it from the iterator and append
                            text_run.push_str(next_text);
                            run_width += next_item.width;
                            items_iter.next(); // Consume peeked item
                        } else {
                            break; // Style or link changed, end of run
                        }
                    } else {
                        break; // Next item is not text, end of run
                    }
                }

                // Create a single element for the entire text run
                let trimmed_text = text_run.trim_end();
                if !trimmed_text.is_empty() {
                    elements.push(PositionedElement {
                        x: current_x,
                        y: start_y,
                        width: run_width, // Use the width of the full run
                        height: style.line_height,
                        element: LayoutElement::Text(TextElement {
                            content: trimmed_text.to_string(), // The combined text
                            href,
                        }),
                        style,
                    });
                }
                current_x += run_width;
            }
            LineContent::Image {
                src,
                data,
                width,
                height,
            } => {
                if let Some(image_data) = data {
                    // Vertically align the image with the text baseline
                    let y_offset = parent_style.line_height - height;
                    elements.push(PositionedElement {
                        x: current_x,
                        y: start_y + y_offset,
                        width,
                        height,
                        element: LayoutElement::Image(ImageElement {
                            src,
                            image_data,
                        }),
                        style: item.style,
                    });
                }
                current_x += item_width;
            }
        }
    }

    parent_style.line_height
}

/// Traverses inline nodes to produce a flat list of items with their computed styles.
fn flatten_inlines(
    engine: &LayoutEngine,
    inlines: &[InlineNode],
    parent_style: &Arc<ComputedStyle>,
    parent_href: Option<&String>,
) -> Vec<LineItem> {
    // PERF: Pre-allocate with capacity. The length of inlines is a good lower bound.
    let mut items = Vec::with_capacity(inlines.len());
    for inline in inlines {
        match inline {
            InlineNode::Text(text) => {
                // Split text by newlines to handle them as explicit line breaks
                let mut parts = text.split('\n').peekable();
                while let Some(part) = parts.next() {
                    if !part.is_empty() {
                        let text_width = engine.measure_text_width(part, parent_style);
                        items.push(LineItem {
                            content: LineContent::Text(part.to_string()),
                            width: text_width,
                            style: parent_style.clone(),
                            href: parent_href.cloned(),
                        });
                    }

                    if parts.peek().is_some() {
                        // If there's another part, it means there was a newline
                        items.push(LineItem {
                            content: LineContent::Text("\n".to_string()),
                            width: 0.0,
                            style: parent_style.clone(),
                            href: None,
                        });
                    }
                }
            }
            InlineNode::StyledSpan {
                style_name,
                style_override,
                children,
            } => {
                let style =
                    engine.compute_style(style_name.as_deref(), style_override.as_ref(), parent_style);
                items.extend(flatten_inlines(engine, children, &style, parent_href));
            }
            InlineNode::Hyperlink {
                href,
                style_name,
                style_override,
                children,
            } => {
                let mut style_arc = engine.compute_style(
                    style_name.as_deref(),
                    style_override.as_ref(),
                    parent_style,
                );
                let style_mut = Arc::make_mut(&mut style_arc);
                style_mut.color = Color {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 1.0,
                }; // Simple link styling
                items.extend(flatten_inlines(engine, children, &style_arc, Some(href)));
            }
            InlineNode::Image {
                src,
                data,
                style_name,
                style_override,
            } => {
                let style =
                    engine.compute_style(style_name.as_deref(), style_override.as_ref(), parent_style);
                let height = style.height.unwrap_or(style.line_height * 0.8);
                let width = style.width.as_ref().map_or(height, |d| match d {
                    Dimension::Pt(w) => *w,
                    _ => height,
                });

                items.push(LineItem {
                    content: LineContent::Image {
                        src: src.clone(),
                        data: data.clone(),
                        width,
                        height,
                    },
                    width,
                    style,
                    href: parent_href.cloned(),
                });
            }
            InlineNode::LineBreak => {
                items.push(LineItem {
                    content: LineContent::Text("\n".to_string()),
                    width: 0.0,
                    style: parent_style.clone(),
                    href: None,
                });
            }
        }
    }
    items
}

/// Lays out a full paragraph node, handling pagination.
/// This function is the public API for laying out a paragraph, called from the page layout dispatcher.
pub fn layout_paragraph_node(
    engine: &LayoutEngine,
    children: &[InlineNode],
    style_name: &Option<String>,
    style_override: &Option<ElementStyle>,
    style: &Arc<ComputedStyle>,
    available_width: f32,
    available_height: f32,
) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
    let content_width = available_width - style.padding.left - style.padding.right;
    let max_height_for_text =
        available_height - style.padding.top - style.padding.bottom - style.margin.bottom;

    let (els, height, remaining_inlines) = layout_paragraph(
        engine,
        children,
        style,
        content_width,
        max_height_for_text.max(0.0),
    );

    let pending_work = remaining_inlines.map(|rem| {
        WorkItem::Node(IRNode::Paragraph {
            style_name: style_name.clone(),
            style_override: style_override.clone(),
            children: rem,
        })
    });

    (els, height, pending_work)
}

/// Measures the width of a text string based on its style.
pub fn measure_text_width(_engine: &LayoutEngine, text: &str, style: &Arc<ComputedStyle>) -> f32 {
    let char_width = style.font_size * 0.6; // Approximation
    text.chars().count() as f32 * char_width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::style;
    use crate::stylesheet::Stylesheet;

    // A mock engine for testing purposes
    fn create_test_engine() -> LayoutEngine {
        LayoutEngine::new(Stylesheet::default())
    }

    #[test]
    fn test_simple_line_break() {
        let engine = create_test_engine();
        let style = style::get_default_style(); // font_size: 12.0, line_height: 14.4
        let inlines =
            vec![InlineNode::Text("This is a long line of text that should wrap.".to_string())];

        let (elements, height, remainder) =
            layout_paragraph(&engine, &inlines, &style, 200.0, 1000.0);

        assert!(remainder.is_none());
        assert_eq!(elements.len(), 2, "Text should have wrapped into exactly two lines");
        let first_y = elements[0].y;
        let second_y = elements[1].y;
        assert!(second_y > first_y, "Second line's Y position should be greater than the first");
        assert!(
            (second_y - first_y).abs() - style.line_height < 0.01,
            "Second line should be one line_height below the first"
        );

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

        let (elements, height, remainder) =
            layout_paragraph(&engine, &inlines, &style, 500.0, 1000.0);

        assert!(remainder.is_none());
        assert_eq!(elements.len(), 2, "There should be exactly two PositionedElements for two lines");
        assert_eq!(elements[0].y, 0.0);
        assert!((elements[1].y - 14.4).abs() < 0.01, "Second line should be one line_height below the first");
        assert!((height - 2.0 * 14.4).abs() < 0.01, "Paragraph height should match two line heights");
    }

    #[test]
    fn test_text_alignment() {
        let engine = create_test_engine();
        let mut style_arc = style::get_default_style();
        let style_mut = Arc::make_mut(&mut style_arc);
        style_mut.text_align = TextAlign::Center;
        let style = style_arc;
        let inlines = vec![InlineNode::Text("Centered".to_string())];

        let available_width = 500.0;
        let (elements, _, _) =
            layout_paragraph(&engine, &inlines, &style, available_width, 1000.0);

        let text_width = measure_text_width(&engine, "Centered", &style);
        let expected_x = (available_width - text_width) / 2.0;

        assert!(!elements.is_empty());
        assert!((elements[0].x - expected_x).abs() < 0.01);
    }
}