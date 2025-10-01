// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/paragraph.rs
use crate::core::idf::IRNode;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::text::{atomize_inlines, LayoutAtom};
use crate::core::layout::{LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::text::TextAlign;
use std::any::Any;
use std::sync::Arc;

/// A `LayoutNode` implementation for paragraphs, capable of line-breaking and page-splitting.
#[derive(Debug)]
pub struct ParagraphNode {
    atoms: Vec<LayoutAtom>,
    style: Arc<ComputedStyle>,
}

impl ParagraphNode {
    /// Creates a new `ParagraphNode` from an `IRNode::Paragraph`.
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Self {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let inlines = match node {
            IRNode::Paragraph { children, .. } => children,
            _ => panic!("ParagraphNode must be created from an IRNode::Paragraph"),
        };
        let atoms = atomize_inlines(engine, inlines, &style, None);
        Self { atoms, style }
    }
}

impl LayoutNode for ParagraphNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn measure_content_height(&mut self, _engine: &LayoutEngine, available_width: f32) -> f32 {
        if let Some(Dimension::Pt(h)) = self.style.height {
            return self.style.margin.top + h + self.style.margin.bottom;
        }

        if self.atoms.is_empty() {
            return self.style.margin.top + self.style.margin.bottom;
        }

        let mut remaining_atoms = self.atoms.as_slice();
        let mut total_height = 0.0;
        let line_height = self.style.line_height;

        while !remaining_atoms.is_empty() {
            let mut line_buffer_width = 0.0;
            let mut break_idx = 0;

            for (i, atom) in remaining_atoms.iter().enumerate() {
                if let LayoutAtom::LineBreak = atom {
                    break_idx = i + 1;
                    break;
                }
                if line_buffer_width > 0.0 && (line_buffer_width + atom.width()) > available_width {
                    break_idx = i;
                    break;
                }
                line_buffer_width += atom.width();
                break_idx = i + 1;
            }

            total_height += line_height;
            remaining_atoms = &remaining_atoms[break_idx..];
        }

        self.style.margin.top + total_height + self.style.margin.bottom
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        if self.atoms.is_empty() {
            return Ok(LayoutResult::Full);
        }

        let total_margin = self.style.margin.top + self.style.margin.bottom;
        if total_margin > ctx.available_height() && !ctx.is_empty() {
            return Ok(LayoutResult::Partial(Box::new(ParagraphNode {
                atoms: std::mem::take(&mut self.atoms),
                style: self.style.clone(),
            })));
        }

        ctx.advance_cursor(self.style.margin.top);

        let available_width = ctx.bounds.width;
        let mut remaining_atoms = self.atoms.as_slice();
        let mut first_line_on_page = true;

        loop {
            let line_height = self.style.line_height;
            if line_height > ctx.available_height() && !first_line_on_page {
                let remainder = Box::new(ParagraphNode {
                    atoms: remaining_atoms.to_vec(),
                    style: self.style.clone(),
                });
                // Before returning, we need to account for the parent's bottom margin
                ctx.advance_cursor(self.style.margin.bottom);
                return Ok(LayoutResult::Partial(remainder));
            }

            let mut line_buffer: Vec<LayoutAtom> = Vec::with_capacity(30);
            let mut current_line_width = 0.0;
            let mut break_idx = 0;

            for (i, atom) in remaining_atoms.iter().enumerate() {
                if let LayoutAtom::LineBreak = atom {
                    break_idx = i + 1;
                    break;
                }
                if line_buffer.is_empty() && atom.is_space() {
                    break_idx = i + 1;
                    continue;
                }
                if !line_buffer.is_empty() && (current_line_width + atom.width()) > available_width {
                    break_idx = i;
                    break;
                }
                line_buffer.push(atom.clone());
                current_line_width += atom.width();
                break_idx = i + 1;
            }

            if line_buffer.is_empty() && !remaining_atoms.is_empty() {
                if let LayoutAtom::Word { .. } | LayoutAtom::Image { .. } = &remaining_atoms[0] {
                    line_buffer.push(remaining_atoms[0].clone());
                    break_idx = 1;
                }
            }

            if line_buffer.is_empty() {
                break; // No more content to lay out
            }

            let is_last_line_of_paragraph = break_idx >= remaining_atoms.len();
            commit_line_to_context(
                ctx,
                self.style.clone(),
                line_buffer,
                available_width,
                is_last_line_of_paragraph,
            );
            ctx.advance_cursor(line_height);
            first_line_on_page = false;

            if break_idx >= remaining_atoms.len() {
                break;
            }
            remaining_atoms = &remaining_atoms[break_idx..];
        }

        ctx.advance_cursor(self.style.margin.bottom);
        Ok(LayoutResult::Full)
    }
}

/// Commits a line of atoms to the LayoutContext.
fn commit_line_to_context(
    ctx: &mut LayoutContext,
    parent_style: Arc<ComputedStyle>,
    mut line_atoms: Vec<LayoutAtom>,
    box_width: f32,
    is_last_line: bool,
) {
    if line_atoms.is_empty() {
        return;
    }
    while line_atoms.last().map_or(false, |a| a.is_space()) {
        line_atoms.pop();
    }
    if line_atoms.is_empty() {
        return;
    }
    let total_content_width: f32 = line_atoms.iter().map(|a| a.width()).sum();
    let justify = !is_last_line && parent_style.text_align == TextAlign::Justify;
    let space_count = if justify { line_atoms.iter().filter(|a| a.is_space()).count() } else { 0 };
    let justification_space = if justify && space_count > 0 && total_content_width < box_width {
        (box_width - total_content_width) / space_count as f32
    } else {
        0.0
    };
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
                    let (current_style, current_href, text_part, part_width) = match &line_atoms[i] {
                        LayoutAtom::Word { text, width, style, href } => (style, href, text.as_str(), *width),
                        LayoutAtom::Space { width, style, .. } => (style, &None, " ", *width),
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
                ctx.push_element(PositionedElement {
                    x: current_x,
                    y: 0.0,
                    width: run_width,
                    height: base_style.line_height,
                    element: crate::core::layout::LayoutElement::Text(crate::core::layout::TextElement {
                        content: run_text,
                        href: base_href.clone(),
                    }),
                    style: base_style.clone(),
                });
                current_x += run_width;
                atom_idx = run_end_idx + 1;
            }
            LayoutAtom::Image {
                src,
                width,
                height,
                style,
                ..
            } => {
                let y_offset = parent_style.line_height - height;
                ctx.push_element(PositionedElement {
                    x: current_x,
                    y: y_offset,
                    width: *width,
                    height: *height,
                    element: crate::core::layout::LayoutElement::Image(crate::core::layout::ImageElement { src: src.clone() }),
                    style: style.clone(),
                });
                current_x += width;
                atom_idx += 1;
            }
            LayoutAtom::LineBreak => {
                atom_idx += 1;
            }
        }
    }
}