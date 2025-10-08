// FILE: /home/sigmund/RustroverProjects/petty/src/core/layout/nodes/paragraph.rs
use crate::core::idf::IRNode;
use crate::core::layout::node::{LayoutContext, LayoutNode, LayoutResult};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::text::{atomize_inlines, LayoutAtom};
use crate::core::layout::{LayoutEngine, LayoutError, PositionedElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::text::TextAlign;
use std::any::Any;
use std::ops::Range;
use std::sync::Arc;

/// A `LayoutNode` implementation for paragraphs, capable of line-breaking and page-splitting.
#[derive(Debug, Clone)]
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

    /// Prepends text to the paragraph's atoms, useful for list markers.
    pub fn prepend_text(&mut self, text: &str, engine: &LayoutEngine) {
        let word = LayoutAtom::Word {
            text: text.to_string(),
            width: engine.measure_text_width(text, &self.style),
            style: self.style.clone(),
            href: None,
        };
        self.atoms.insert(0, word);
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
        let lines = break_atoms_into_line_ranges(&self.atoms, available_width);
        let content_height = lines.len() as f32 * self.style.line_height;

        self.style.margin.top + content_height + self.style.margin.bottom
    }

    fn measure_intrinsic_width(&self, _engine: &LayoutEngine) -> f32 {
        // The max-content width is the sum of all atom widths on a single line.
        // This simple sum is a good approximation for flex basis calculation.
        self.atoms.iter().map(|a| a.width()).sum()
    }

    fn layout(&mut self, ctx: &mut LayoutContext) -> Result<LayoutResult, LayoutError> {
        // --- Vertical Margin Collapsing ---
        let margin_to_add = self.style.margin.top.max(ctx.last_v_margin);

        if self.atoms.is_empty() {
            ctx.advance_cursor(margin_to_add);
            ctx.last_v_margin = self.style.margin.bottom;
            return Ok(LayoutResult::Full);
        }

        if !ctx.is_empty() && margin_to_add > ctx.available_height() {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }

        // Apply the collapsed margin by advancing the cursor.
        ctx.advance_cursor(margin_to_add);
        // The margin has been "used," so reset it for any subsequent children.
        ctx.last_v_margin = 0.0;

        let available_width = ctx.bounds.width;
        let all_line_ranges = break_atoms_into_line_ranges(&self.atoms, available_width);
        let total_lines = all_line_ranges.len();
        let line_height = self.style.line_height;

        let mut lines_that_fit = (ctx.available_height() / line_height).floor() as usize;
        if ctx.is_empty() && lines_that_fit == 0 && total_lines > 0 {
            lines_that_fit = 1;
        }
        lines_that_fit = lines_that_fit.min(total_lines);

        // --- Orphans Control ---
        // If a break would occur, and it would leave fewer than `orphans` lines on this page,
        // force the entire paragraph to the next page.
        if !ctx.is_empty() && total_lines > lines_that_fit && lines_that_fit < self.style.orphans {
            return Ok(LayoutResult::Partial(Box::new(self.clone())));
        }

        // --- Widows Control ---
        if lines_that_fit < total_lines && lines_that_fit > 0 {
            let remaining_lines_count = total_lines - lines_that_fit;
            if remaining_lines_count < self.style.widows {
                let lines_to_move = (self.style.widows - remaining_lines_count).min(lines_that_fit);
                lines_that_fit -= lines_to_move;
            }
        }

        for i in 0..lines_that_fit {
            let range = &all_line_ranges[i];
            let line_atoms = self.atoms[range.clone()].to_vec();
            let is_last_line_of_paragraph = i == total_lines - 1;
            commit_line_to_context(
                ctx,
                self.style.clone(),
                line_atoms,
                available_width,
                is_last_line_of_paragraph,
            );
            ctx.advance_cursor(line_height);
        }

        if lines_that_fit >= total_lines {
            ctx.last_v_margin = self.style.margin.bottom;
            Ok(LayoutResult::Full)
        } else {
            let remainder_start_idx = all_line_ranges[lines_that_fit].start;
            let remainder_atoms = self.atoms[remainder_start_idx..].to_vec();
            let remainder = Box::new(ParagraphNode {
                atoms: remainder_atoms,
                style: self.style.clone(),
            });
            Ok(LayoutResult::Partial(remainder))
        }
    }
}

fn break_atoms_into_line_ranges(atoms: &[LayoutAtom], available_width: f32) -> Vec<Range<usize>> {
    if atoms.is_empty() {
        return vec![];
    }
    let mut lines = Vec::new();
    let mut current_pos = 0;

    while current_pos < atoms.len() {
        let mut line_start = current_pos;
        while line_start < atoms.len() && atoms[line_start].is_space() {
            line_start += 1;
        }
        if line_start >= atoms.len() {
            break;
        }

        let mut line_buffer_width = 0.0;
        let mut potential_break_idx = line_start;
        let mut line_end = line_start;
        let mut next_pos = line_start;

        for i in line_start..atoms.len() {
            let atom = &atoms[i];
            if let LayoutAtom::LineBreak = atom {
                line_end = i;
                next_pos = i + 1;
                break;
            }

            if !atom.is_space() && line_buffer_width > 0.0 && (line_buffer_width + atom.width()) > available_width {
                if potential_break_idx > line_start {
                    line_end = potential_break_idx;
                    next_pos = potential_break_idx;
                } else {
                    line_end = i;
                    next_pos = i;
                }
                break;
            }
            line_buffer_width += atom.width();
            if atom.is_space() {
                potential_break_idx = i;
            }
            line_end = i + 1;
            next_pos = i + 1;
        }

        lines.push(line_start..line_end);
        current_pos = next_pos;
    }
    lines
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
                        text_decoration: base_style.text_decoration.clone(),
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