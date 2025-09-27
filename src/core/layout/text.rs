//! Text measurement and paragraph layout with line breaking.

use super::elements::{ImageElement, LayoutElement, PositionedElement, TextElement};
use super::style::ComputedStyle;
use super::{ LayoutEngine, WorkItem};
use std::sync::Arc;
use crate::core::idf::{IRNode, InlineNode};
use crate::core::style::color::Color;
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::ElementStyle;
use crate::core::style::text::TextAlign;

/// Measures the width of a string of text with a given style.
pub fn measure_text_width(engine: &LayoutEngine, text: &str, style: &Arc<ComputedStyle>) -> f32 {
    engine.measure_text_width(text, style)
}

/// Represents a piece of content that can be placed on a line (text or image).
#[derive(Debug, Clone)]
enum LineContent {
    Text(String),
    Image {
        src: String,
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
        .collect::<Vec<_>>()
        .join(" "); // Re-join the remaining words with spaces.

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
    let mut elements = Vec::with_capacity(line_items.len() / 5);
    let mut line_buffer = Vec::with_capacity(20);

    let box_width = available_width;
    let mut current_y = 0.0;
    let mut current_line_width = 0.0;

    // FIX: Pre-calculate space width for more robust line breaking.
    let space_width = measure_text_width(engine, " ", style);

    let mut all_items = line_items;
    let mut item_idx = 0;

    while item_idx < all_items.len() {
        let item = &all_items[item_idx];

        if let LineContent::Text(text) = &item.content {
            if text == "\n" {
                if current_y + style.line_height > max_height && !elements.is_empty() {
                    let pending = all_items.split_off(item_idx);
                    return (elements, current_y, Some(convert_items_to_inlines(pending)));
                }
                current_y += commit_line(&mut elements, engine, style, std::mem::take(&mut line_buffer), box_width, current_y);
                current_line_width = 0.0;
                item_idx += 1;
                continue;
            }
        }

        // FIX: Reworked word-wrapping logic to use `split_whitespace` and manual space handling.
        match &item.content {
            LineContent::Text(text) => {
                let words = text.split_whitespace().collect::<Vec<_>>();
                let mut word_idx = 0;
                while word_idx < words.len() {
                    let word_str = words[word_idx];
                    let word_width = measure_text_width(engine, word_str, &item.style);

                    let effective_width = if line_buffer.is_empty() { word_width } else { space_width + word_width };

                    if !line_buffer.is_empty() && (current_line_width + effective_width) > box_width {
                        if current_y + style.line_height > max_height {
                            let remaining_words = words[word_idx..].join(" ");
                            let mut pending_items = Vec::with_capacity(1 + all_items.len().saturating_sub(item_idx + 1));
                            pending_items.push(LineItem {
                                content: LineContent::Text(remaining_words),
                                width: 0.0,
                                style: item.style.clone(),
                                href: item.href.clone(),
                            });
                            if item_idx + 1 < all_items.len() {
                                pending_items.extend(all_items[(item_idx + 1)..].iter().cloned());
                            }
                            return (elements, current_y, Some(convert_items_to_inlines(pending_items)));
                        }
                        current_y += commit_line(&mut elements, engine, style, std::mem::take(&mut line_buffer), box_width, current_y);
                        current_line_width = 0.0;
                    }

                    line_buffer.push(LineItem {
                        content: LineContent::Text(word_str.to_string()),
                        width: word_width,
                        style: item.style.clone(),
                        href: item.href.clone(),
                    });
                    current_line_width += if current_line_width == 0.0 { word_width } else { space_width + word_width };
                    word_idx += 1;
                }
            }
            LineContent::Image { .. } => {
                let effective_width = if line_buffer.is_empty() { item.width } else { space_width + item.width };
                if !line_buffer.is_empty() && (current_line_width + effective_width) > box_width {
                    if current_y + style.line_height > max_height {
                        let pending_items = all_items.split_off(item_idx);
                        return (elements, current_y, Some(convert_items_to_inlines(pending_items)));
                    }
                    current_y += commit_line(&mut elements, engine, style, std::mem::take(&mut line_buffer), box_width, current_y);
                    current_line_width = 0.0;
                }
                line_buffer.push(item.clone());
                current_line_width += if current_line_width == 0.0 { item.width } else { space_width + item.width };
            }
        }
        item_idx += 1;
    }

    if !line_buffer.is_empty() {
        if current_y + style.line_height > max_height && !elements.is_empty() {
            return (elements, current_y, Some(convert_items_to_inlines(line_buffer)));
        }
        current_y += commit_line(&mut elements, engine, style, line_buffer, box_width, current_y);
    }

    (elements, current_y, None)
}

/// A cheap, measurement-only version of layout_paragraph.
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
    // FIX: Use consistent logic with layout_paragraph.
    let space_width = measure_text_width(engine, " ", style);

    for item in &line_items {
        if let LineContent::Text(text) = &item.content {
            if text == "\n" {
                line_count += 1;
                current_line_width = 0.0;
                continue;
            }
        }

        if let LineContent::Text(text) = &item.content {
            for word_str in text.split_whitespace() {
                let word_width = measure_text_width(engine, word_str, &item.style);
                let effective_width = if current_line_width == 0.0 { word_width } else { space_width + word_width };

                if current_line_width > 0.0 && (current_line_width + effective_width) > box_width {
                    line_count += 1;
                    current_line_width = 0.0;
                }
                current_line_width += if current_line_width == 0.0 { word_width } else { space_width + word_width };
            }
        } else {
            let effective_width = if current_line_width == 0.0 { item.width } else { space_width + item.width };
            if current_line_width > 0.0 && (current_line_width + effective_width) > box_width {
                line_count += 1;
                current_line_width = 0.0;
            }
            current_line_width += if current_line_width == 0.0 { item.width } else { space_width + item.width };
        }
    }

    line_count as f32 * style.line_height
}

/// Helper to position and generate elements for a single line of content.
fn commit_line(
    elements: &mut Vec<PositionedElement>,
    engine: &LayoutEngine,
    parent_style: &Arc<ComputedStyle>,
    line_items: Vec<LineItem>,
    box_width: f32,
    start_y: f32,
) -> f32 {
    if line_items.is_empty() {
        return parent_style.line_height;
    }

    // FIX: Calculate total width based on clean word widths and spaces between them.
    let space_width = measure_text_width(engine, " ", parent_style);
    let total_content_width: f32 = line_items.iter().map(|item| item.width).sum::<f32>()
        + if line_items.len() > 1 { space_width * (line_items.len() - 1) as f32 } else { 0.0 };

    let mut current_x = match parent_style.text_align {
        TextAlign::Left => 0.0,
        TextAlign::Center => (box_width - total_content_width) / 2.0,
        TextAlign::Right => box_width - total_content_width,
        TextAlign::Justify => 0.0,
    };

    let mut items_iter = line_items.into_iter().peekable();

    while let Some(item) = items_iter.next() {
        match item.content {
            LineContent::Text(text) => {
                elements.push(PositionedElement {
                    x: current_x,
                    y: start_y,
                    width: item.width,
                    height: item.style.line_height,
                    element: LayoutElement::Text(TextElement {
                        content: text,
                        href: item.href,
                    }),
                    style: item.style,
                });
                current_x += item.width;
            }
            LineContent::Image { src, width, height } => {
                let y_offset = parent_style.line_height - height;
                elements.push(PositionedElement {
                    x: current_x,
                    y: start_y + y_offset,
                    width,
                    height,
                    element: LayoutElement::Image(ImageElement { src }),
                    style: item.style,
                });
                current_x += width;
            }
        }

        // Add a space after the item if it's not the last one.
        if items_iter.peek().is_some() {
            current_x += space_width;
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
    let mut items = Vec::with_capacity(inlines.len());
    for inline in inlines {
        match inline {
            InlineNode::Text(text) => {
                let mut parts = text.split('\n').peekable();
                while let Some(part) = parts.next() {
                    if !part.is_empty() {
                        let text_width = measure_text_width(engine, part, parent_style);
                        items.push(LineItem {
                            content: LineContent::Text(part.to_string()),
                            width: text_width,
                            style: parent_style.clone(),
                            href: parent_href.cloned(),
                        });
                    }

                    if parts.peek().is_some() {
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
                style_sets,
                style_override,
                children,
            } => {
                let style =
                    engine.compute_style(style_sets, style_override.as_ref(), parent_style);
                items.extend(flatten_inlines(engine, children, &style, parent_href));
            }
            InlineNode::Hyperlink {
                href,
                style_sets,
                style_override,
                children,
            } => {
                let mut style_arc =
                    engine.compute_style(style_sets, style_override.as_ref(), parent_style);
                let style_mut = Arc::make_mut(&mut style_arc);
                style_mut.color = Color {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 1.0,
                };
                items.extend(flatten_inlines(engine, children, &style_arc, Some(href)));
            }
            InlineNode::Image {
                src,
                style_sets,
                style_override,
            } => {
                let style =
                    engine.compute_style(style_sets, style_override.as_ref(), parent_style);
                let height = style.height.unwrap_or(style.line_height * 0.8);
                let width = style.width.as_ref().map_or(height, |d| match d {
                    Dimension::Pt(w) => *w,
                    _ => height,
                });

                items.push(LineItem {
                    content: LineContent::Image {
                        src: src.clone(),
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
pub fn layout_paragraph_node(
    engine: &LayoutEngine,
    children: &mut [InlineNode],
    style_sets: &[Arc<ElementStyle>],
    style_override: &Option<ElementStyle>,
    style: &Arc<ComputedStyle>,
    available_width: f32,
    available_height: f32,
) -> (Vec<PositionedElement>, f32, Option<WorkItem>) {
    let max_height_for_text =
        available_height - style.padding.top - style.padding.bottom - style.margin.bottom;

    let (els, height, remaining_inlines) = layout_paragraph(
        engine,
        children,
        style,
        available_width,
        max_height_for_text.max(0.0),
    );

    let pending_work = remaining_inlines.map(|rem| {
        WorkItem::Node(IRNode::Paragraph {
            style_sets: style_sets.to_vec(),
            style_override: style_override.clone(),
            children: rem,
        })
    });

    (els, height, pending_work)
}

// --- Subtree Layout ---

/// Lays out a paragraph for a subtree measurement.
pub(super) fn layout_paragraph_subtree(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> (Vec<PositionedElement>, f32) {
    let children = match node {
        IRNode::Paragraph { children, .. } => children,
        _ => return (vec![], 0.0),
    };

    let (els, height, _remainder) =
        layout_paragraph(engine, children, style, content_width, f32::MAX);
    (els, height + style.padding.top + style.padding.bottom)
}

/// Measures a paragraph for a subtree measurement.
pub(super) fn measure_paragraph_subtree(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: &Arc<ComputedStyle>,
    content_width: f32,
) -> f32 {
    let children = match node {
        IRNode::Paragraph { children, .. } => children,
        _ => return 0.0,
    };
    let height = measure_paragraph_height(engine, children, style, content_width);
    height + style.padding.top + style.padding.bottom
}