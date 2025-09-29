//! Text measurement and paragraph layout with line breaking.

use super::style::ComputedStyle;
use super::{LayoutBox, LayoutContent, LayoutEngine, Rect};
use crate::core::idf::{IRNode, InlineNode};
use crate::core::style::color::Color;
use crate::core::style::dimension::Dimension;
use crate::core::style::text::TextAlign;
use std::sync::Arc;

/// The smallest unbreakable unit for inline layout.
#[derive(Debug, Clone)]
pub enum LayoutAtom {
    Word {
        text: String,
        width: f32,
        style: Arc<ComputedStyle>,
        href: Option<String>,
    },
    Space {
        width: f32,
        style: Arc<ComputedStyle>,
    },
    Image {
        src: String,
        width: f32,
        height: f32,
        style: Arc<ComputedStyle>,
        href: Option<String>,
    },
    LineBreak,
}

impl LayoutAtom {
    pub(crate) fn width(&self) -> f32 {
        match self {
            LayoutAtom::Word { width, .. } => *width,
            LayoutAtom::Space { width, .. } => *width,
            LayoutAtom::Image { width, .. } => *width,
            LayoutAtom::LineBreak => 0.0,
        }
    }

    fn is_space(&self) -> bool {
        matches!(self, LayoutAtom::Space { .. })
    }
}

/// Lays out a paragraph with line breaking.
pub fn layout_paragraph(
    engine: &LayoutEngine,
    node: &mut IRNode,
    style: Arc<ComputedStyle>,
    available_size: (f32, f32),
) -> LayoutBox {
    let inlines = match node {
        IRNode::Paragraph { children, .. } => children,
        _ => {
            return LayoutBox {
                rect: Rect::default(),
                style,
                content: LayoutContent::Children(vec![]),
            }
        }
    };

    let atoms = atomize_inlines(engine, inlines, &style, None);
    if atoms.is_empty() {
        return LayoutBox {
            rect: Rect::default(),
            style,
            content: LayoutContent::Children(vec![]),
        };
    }

    let mut line_boxes = Vec::new();
    let mut line_buffer: Vec<LayoutAtom> = Vec::with_capacity(30);
    let mut current_y = 0.0;
    let mut current_line_width = 0.0;
    let mut atoms_iter = atoms.into_iter().peekable();

    while atoms_iter.peek().is_some() {
        let atom = atoms_iter.peek().unwrap(); // Safe due to is_some() check

        // --- Line Breaking Logic ---
        // If the line has content and the next atom doesn't fit, commit the line.
        if !line_buffer.is_empty() && (current_line_width + atom.width()) > available_size.0 {
            let is_last_line = atoms_iter.peek().is_none();
            let (new_boxes, line_height) = commit_line(
                style.clone(),
                std::mem::take(&mut line_buffer),
                available_size.0,
                current_y,
                is_last_line,
            );
            line_boxes.extend(new_boxes);
            current_y += line_height;
            current_line_width = 0.0;
            // Do not advance the iterator; the atom that didn't fit will be processed
            // on the new, empty line in the next iteration.
            continue;
        }

        // --- Atom Consumption Logic ---
        let consumed_atom = atoms_iter.next().unwrap(); // Consume the atom

        // Handle hard line breaks
        if let LayoutAtom::LineBreak = consumed_atom {
            let is_last_line = atoms_iter.peek().is_none();
            let (new_boxes, line_height) = commit_line(
                style.clone(),
                std::mem::take(&mut line_buffer),
                available_size.0,
                current_y,
                is_last_line,
            );
            line_boxes.extend(new_boxes);
            current_y += line_height;
            current_line_width = 0.0;
            continue;
        }

        // Don't start a line with a space.
        if line_buffer.is_empty() && consumed_atom.is_space() {
            continue; // Discard and move to next atom.
        }

        // Add the atom to the current line.
        current_line_width += consumed_atom.width();
        line_buffer.push(consumed_atom);
    }

    // Commit any remaining atoms in the buffer as the final line.
    if !line_buffer.is_empty() {
        let (new_boxes, line_height) =
            commit_line(style.clone(), line_buffer, available_size.0, current_y, true);
        line_boxes.extend(new_boxes);
        current_y += line_height;
    }

    LayoutBox {
        rect: Rect {
            height: current_y,
            ..Default::default()
        },
        style,
        content: LayoutContent::Children(line_boxes),
    }
}

/// Commits a line of atoms, coalescing them into `LayoutBox` runs.
fn commit_line(
    parent_style: Arc<ComputedStyle>,
    mut line_atoms: Vec<LayoutAtom>,
    box_width: f32,
    start_y: f32,
    is_last_line: bool,
) -> (Vec<LayoutBox>, f32) {
    if line_atoms.is_empty() {
        return (vec![], parent_style.line_height);
    }

    // Trim trailing space
    while let Some(last) = line_atoms.last() {
        if last.is_space() {
            line_atoms.pop();
        } else {
            break;
        }
    }

    let mut boxes = vec![];
    let total_content_width: f32 = line_atoms.iter().map(|a| a.width()).sum();

    let justify = !is_last_line && parent_style.text_align == TextAlign::Justify;
    let space_count = if justify {
        line_atoms.iter().filter(|a| a.is_space()).count()
    } else {
        0
    };

    let mut justification_space = 0.0;
    if justify && space_count > 0 && total_content_width < box_width {
        justification_space = (box_width - total_content_width) / space_count as f32;
    }

    let mut current_x = match parent_style.text_align {
        TextAlign::Left | TextAlign::Justify => 0.0,
        TextAlign::Center => (box_width - total_content_width).max(0.0) / 2.0,
        TextAlign::Right => (box_width - total_content_width).max(0.0),
    };

    let mut atom_idx = 0;
    while atom_idx < line_atoms.len() {
        let atom = &line_atoms[atom_idx];

        match atom {
            LayoutAtom::Word { .. } | LayoutAtom::Space { .. } => {
                let mut run_text = String::new();
                let mut run_width = 0.0;
                let (base_style, base_href) = match atom {
                    LayoutAtom::Word { style, href, .. } => (style, href),
                    LayoutAtom::Space { style, .. } => (style, &None),
                    _ => unreachable!(),
                };

                let mut run_end_idx = atom_idx;
                for i in atom_idx..line_atoms.len() {
                    let (current_style, current_href, text_part, part_width) =
                        match &line_atoms[i] {
                            LayoutAtom::Word { text, width, style, href } => (style, href, text.as_str(), *width),
                            LayoutAtom::Space { width, style } => (style, &None, " ", *width),
                            _ => break,
                        };
                    if Arc::ptr_eq(current_style, base_style) && current_href == base_href {
                        run_text.push_str(text_part);
                        run_width += part_width;
                        if line_atoms[i].is_space() {
                            run_width += justification_space;
                        }
                        run_end_idx = i;
                    } else {
                        break;
                    }
                }

                boxes.push(LayoutBox {
                    rect: Rect {
                        x: current_x,
                        y: start_y,
                        width: run_width,
                        height: base_style.line_height,
                    },
                    style: base_style.clone(),
                    content: LayoutContent::Text(run_text, base_href.clone()),
                });
                current_x += run_width;
                atom_idx = run_end_idx + 1;
            }
            LayoutAtom::Image { src, width, height, style, .. } => {
                let y_offset = parent_style.line_height - height;
                boxes.push(LayoutBox {
                    rect: Rect {
                        x: current_x,
                        y: start_y + y_offset,
                        width: *width,
                        height: *height,
                    },
                    style: style.clone(),
                    content: LayoutContent::Image(src.clone()),
                });
                current_x += width;
                atom_idx += 1;
            }
            LayoutAtom::LineBreak => {
                atom_idx += 1;
            }
        }
    }

    (boxes, parent_style.line_height)
}

/// Traverses inline nodes to produce a flat list of `LayoutAtom`s.
pub(crate) fn atomize_inlines(
    engine: &LayoutEngine,
    inlines: &[InlineNode],
    parent_style: &Arc<ComputedStyle>,
    parent_href: Option<&String>,
) -> Vec<LayoutAtom> {
    let mut atoms = Vec::new();
    for inline in inlines {
        match inline {
            InlineNode::Text(text) => {
                let space_width = engine.measure_text_width(" ", parent_style);
                for (i, word) in text.split_whitespace().enumerate() {
                    if i > 0 {
                        atoms.push(LayoutAtom::Space {
                            width: space_width,
                            style: parent_style.clone(),
                        });
                    }
                    if !word.is_empty() {
                        let word_width = engine.measure_text_width(word, parent_style);
                        atoms.push(LayoutAtom::Word {
                            text: word.to_string(),
                            width: word_width,
                            style: parent_style.clone(),
                            href: parent_href.cloned(),
                        });
                    }
                }
            }
            InlineNode::StyledSpan { style_sets, style_override, children } => {
                let style = engine.compute_style(style_sets, style_override.as_ref(), parent_style);
                atoms.extend(atomize_inlines(engine, children, &style, parent_href));
            }
            InlineNode::Hyperlink { href, style_sets, style_override, children } => {
                let mut style_arc =
                    engine.compute_style(style_sets, style_override.as_ref(), parent_style);
                let style_mut = Arc::make_mut(&mut style_arc);
                style_mut.color = Color { r: 0, g: 0, b: 255, a: 1.0 };
                atoms.extend(atomize_inlines(engine, children, &style_arc, Some(href)));
            }
            InlineNode::Image { src, style_sets, style_override } => {
                let style = engine.compute_style(style_sets, style_override.as_ref(), parent_style);
                let height = if let Some(Dimension::Pt(h)) = style.height {
                    h
                } else {
                    style.line_height * 0.8
                };
                let width = style.width.as_ref().map_or(height, |d| match d {
                    Dimension::Pt(w) => *w,
                    _ => height,
                });
                atoms.push(LayoutAtom::Image {
                    src: src.clone(),
                    width,
                    height,
                    style,
                    href: parent_href.cloned(),
                });
            }
            InlineNode::LineBreak => {
                atoms.push(LayoutAtom::LineBreak);
            }
        }
    }
    atoms
}

/// A measurement-only version that calculates the paragraph's max-content width,
/// which is its width if it were rendered on a single infinite line.
pub(crate) fn measure_paragraph_max_content_width(
    engine: &LayoutEngine,
    node: &IRNode,
    style: &Arc<ComputedStyle>,
) -> f32 {
    let children = match node {
        IRNode::Paragraph { children, .. } => children,
        _ => return 0.0,
    };
    let atoms = atomize_inlines(engine, children, style, None);
    atoms.iter().map(LayoutAtom::width).sum()
}