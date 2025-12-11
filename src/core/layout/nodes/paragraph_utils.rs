use crate::core::layout::style::ComputedStyle;
use crate::core::layout::{LayoutContext, LayoutElement, LayoutEngine, PositionedElement, TextElement};
use crate::core::layout::text::{TextSpan, InlineImageEntry};
use crate::core::style::text::TextAlign;
use std::sync::Arc;
use rustybuzz::{UnicodeBuffer, Feature};
use ttf_parser::Tag;
use std::cell::RefCell;
use crate::core::layout::node::LayoutNode;

// Reuse buffer to avoid allocations in the tight loop
thread_local! {
    static SCRATCH_BUFFER: RefCell<Option<UnicodeBuffer>> = RefCell::new(Some(UnicodeBuffer::new()));
}

#[derive(Debug, Clone)]
pub struct GlyphInstance {
    pub index: u32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub x_advance: f32,
    pub cluster: u32,
}

#[derive(Debug, Clone)]
pub struct ShapedRun {
    pub glyphs: Vec<GlyphInstance>,
    pub width: f32,
    pub style: Arc<ComputedStyle>,
    pub font_data: Option<crate::core::layout::fonts::FontData>,
    pub font_size: f32,
    pub link_index: usize,
    pub is_image: bool,
    pub text_range: std::ops::Range<usize>,
    pub ascender: f32,
    pub line_height: f32,
    pub baseline_offset: f32,
}

#[derive(Debug, Clone)]
pub struct LineLayout {
    pub items: Vec<LineItem>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

#[derive(Debug, Clone)]
pub struct LineItem {
    pub run_index: usize,
    pub start_glyph: usize,
    pub end_glyph: usize,
    pub x: f32,
    pub width: f32,
}

pub fn shape_text(
    engine: &LayoutEngine,
    spans: &[TextSpan],
    images: &[InlineImageEntry],
) -> Vec<ShapedRun> {
    let mut runs = Vec::new();
    let mut current_char_idx = 0;

    static FEATURES: std::sync::OnceLock<Vec<Feature>> = std::sync::OnceLock::new();
    let features = FEATURES.get_or_init(|| vec![
        Feature::new(Tag::from_bytes(b"liga"), 1, ..),
        Feature::new(Tag::from_bytes(b"kern"), 1, ..)
    ]);

    let mut last_style_ref: Option<&Arc<ComputedStyle>> = None;
    let mut last_font_data: Option<crate::core::layout::fonts::FontData> = None;

    for span in spans {
        let span_len = span.text.len();

        if let Some(img_entry) = images.iter().find(|img| img.index == current_char_idx) {
            let width = if let Some(crate::core::style::dimension::Dimension::Pt(w)) = img_entry.node.style().box_model.width { w } else { 0.0 };
            let height = if let Some(crate::core::style::dimension::Dimension::Pt(h)) = img_entry.node.style().box_model.height { h } else { 0.0 };

            runs.push(ShapedRun {
                glyphs: Vec::new(),
                width,
                style: span.style.clone(),
                font_data: None,
                font_size: 0.0,
                link_index: span.link_index,
                is_image: true,
                text_range: current_char_idx..(current_char_idx + span_len),
                ascender: height,
                line_height: height,
                baseline_offset: height,
            });

            last_style_ref = None;
            last_font_data = None;

            current_char_idx += span_len;
            continue;
        }

        let font_data = if let Some(last) = last_style_ref {
            if **last == *span.style {
                last_font_data.clone()
            } else {
                let fd = engine.get_font_for_style(&span.style)
                    .or_else(|| engine.get_font_for_style(&engine.get_default_style()));
                last_style_ref = Some(&span.style);
                last_font_data = fd.clone();
                fd
            }
        } else {
            let fd = engine.get_font_for_style(&span.style)
                .or_else(|| engine.get_font_for_style(&engine.get_default_style()));
            last_style_ref = Some(&span.style);
            last_font_data = fd.clone();
            fd
        };

        let font_data = match font_data {
            Some(fd) => fd,
            None => {
                current_char_idx += span_len;
                continue;
            }
        };

        // Stack allocation of Face, safe because font_data (Arc<Vec<u8>>) outlives it
        let face = match font_data.as_face() {
            Some(f) => f,
            None => {
                current_char_idx += span_len;
                continue;
            }
        };

        let scale = span.style.text.font_size / face.units_per_em() as f32;
        let ascender = face.ascender() as f32 * scale;
        let style_line_height = span.style.text.line_height;
        let baseline_offset = (style_line_height - (style_line_height - ascender)) / 2.0 + ascender;

        let mut buffer = SCRATCH_BUFFER.with(|b| b.borrow_mut().take().unwrap_or_else(UnicodeBuffer::new));
        buffer.push_str(span.text);
        buffer.guess_segment_properties();

        let glyph_buffer = rustybuzz::shape(&face, features, buffer);

        let infos = glyph_buffer.glyph_infos();
        let positions = glyph_buffer.glyph_positions();

        let mut glyph_instances = Vec::with_capacity(infos.len());
        let mut total_width = 0.0;

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let x_advance = pos.x_advance as f32 * scale;
            glyph_instances.push(GlyphInstance {
                index: info.glyph_id,
                x_offset: pos.x_offset as f32 * scale,
                y_offset: pos.y_offset as f32 * scale,
                x_advance,
                cluster: current_char_idx as u32 + info.cluster,
            });
            total_width += x_advance;
        }

        let recycled_buffer = glyph_buffer.clear();
        SCRATCH_BUFFER.with(|b| *b.borrow_mut() = Some(recycled_buffer));

        runs.push(ShapedRun {
            glyphs: glyph_instances,
            width: total_width,
            style: span.style.clone(),
            font_data: Some(font_data),
            font_size: span.style.text.font_size,
            link_index: span.link_index,
            is_image: false,
            text_range: current_char_idx..(current_char_idx + span_len),
            ascender,
            line_height: style_line_height,
            baseline_offset,
        });

        current_char_idx += span_len;
    }

    runs
}

pub fn break_lines(runs: &[ShapedRun], max_width: f32, block_style: &ComputedStyle, full_text: &str) -> Vec<LineLayout> {
    let mut lines = Vec::new();
    let mut current_line_items = Vec::new();
    let mut current_line_width = 0.0;
    let mut current_line_height = 0.0f32;
    let mut current_line_baseline = 0.0f32;

    for (run_idx, run) in runs.iter().enumerate() {
        if run.is_image {
            if current_line_width + run.width > max_width && !current_line_items.is_empty() {
                lines.push(finalize_line(current_line_items, current_line_width, current_line_height, current_line_baseline, max_width, &block_style.text.text_align, full_text, runs));
                current_line_items = Vec::new();
                current_line_width = 0.0;
                current_line_height = 0.0;
                current_line_baseline = 0.0;
            }
            current_line_items.push(LineItem {
                run_index: run_idx,
                start_glyph: 0,
                end_glyph: 0,
                x: current_line_width,
                width: run.width,
            });
            current_line_width += run.width;
            current_line_height = current_line_height.max(run.line_height);
            continue;
        }

        current_line_height = current_line_height.max(run.line_height);
        current_line_baseline = current_line_baseline.max(run.baseline_offset);

        let mut glyph_start = 0;
        let mut glyph_idx = 0;
        let mut current_segment_width = 0.0;

        while glyph_idx < run.glyphs.len() {
            let glyph = &run.glyphs[glyph_idx];
            let cluster = glyph.cluster as usize;

            let is_newline = cluster < full_text.len() && full_text.as_bytes()[cluster] == b'\n';
            let is_space = cluster < full_text.len() && full_text.as_bytes()[cluster] == b' ';

            let char_width = glyph.x_advance;

            if is_newline {
                if glyph_idx >= glyph_start {
                    current_line_items.push(LineItem {
                        run_index: run_idx,
                        start_glyph: glyph_start,
                        end_glyph: glyph_idx + 1,
                        x: current_line_width,
                        width: current_segment_width + char_width,
                    });
                }

                lines.push(finalize_line(current_line_items, current_line_width + current_segment_width + char_width, current_line_height, current_line_baseline, max_width, &block_style.text.text_align, full_text, runs));

                current_line_items = Vec::new();
                current_line_width = 0.0;
                current_segment_width = 0.0;

                glyph_start = glyph_idx + 1;
                glyph_idx += 1;
                continue;
            }

            if is_space {
                if current_line_width + current_segment_width + char_width > max_width {
                    if !current_line_items.is_empty() {
                        lines.push(finalize_line(current_line_items, current_line_width + current_segment_width, current_line_height, current_line_baseline, max_width, &block_style.text.text_align, full_text, runs));
                        current_line_items = Vec::new();
                        current_line_width = 0.0;
                        current_segment_width = 0.0;
                    }
                }

                if glyph_idx >= glyph_start {
                    current_line_items.push(LineItem {
                        run_index: run_idx,
                        start_glyph: glyph_start,
                        end_glyph: glyph_idx + 1,
                        x: current_line_width,
                        width: current_segment_width + char_width,
                    });
                    current_line_width += current_segment_width + char_width;
                    current_segment_width = 0.0;
                    glyph_start = glyph_idx + 1;
                }

                glyph_idx += 1;
                continue;
            }

            if current_line_width + current_segment_width + char_width > max_width {
                if current_line_items.is_empty() && glyph_start == glyph_idx {
                    current_segment_width += char_width;
                    glyph_idx += 1;
                    continue;
                }

                if glyph_idx > glyph_start {
                    current_line_items.push(LineItem {
                        run_index: run_idx,
                        start_glyph: glyph_start,
                        end_glyph: glyph_idx,
                        x: current_line_width,
                        width: current_segment_width,
                    });
                }

                lines.push(finalize_line(current_line_items, current_line_width + current_segment_width, current_line_height, current_line_baseline, max_width, &block_style.text.text_align, full_text, runs));
                current_line_items = Vec::new();
                current_line_width = 0.0;
                current_segment_width = 0.0;
                glyph_start = glyph_idx;
                continue;
            }

            current_segment_width += char_width;
            glyph_idx += 1;
        }

        if glyph_idx > glyph_start {
            current_line_items.push(LineItem {
                run_index: run_idx,
                start_glyph: glyph_start,
                end_glyph: glyph_idx,
                x: current_line_width,
                width: current_segment_width,
            });
            current_line_width += current_segment_width;
        }
    }

    if !current_line_items.is_empty() {
        lines.push(finalize_line(current_line_items, current_line_width, current_line_height, current_line_baseline, max_width, &block_style.text.text_align, full_text, runs));
    }
    lines
}

fn finalize_line(mut items: Vec<LineItem>, content_width: f32, height: f32, baseline: f32, max_width: f32, align: &TextAlign, full_text: &str, runs: &[ShapedRun]) -> LineLayout {
    if !max_width.is_finite() {
        return LineLayout { items, width: content_width, height, baseline };
    }
    let free_space = (max_width - content_width).max(0.0);
    match align {
        TextAlign::Center => {
            let offset = free_space / 2.0;
            for item in &mut items {
                item.x += offset;
            }
        }
        TextAlign::Right => {
            for item in &mut items {
                item.x += free_space;
            }
        }
        TextAlign::Justify => {
            let mut space_count = 0;
            for item in &items {
                if item.end_glyph > 0 {
                    let run = &runs[item.run_index];
                    if item.end_glyph <= run.glyphs.len() {
                        let last_glyph = &run.glyphs[item.end_glyph - 1];
                        let cluster = last_glyph.cluster as usize;
                        if cluster < full_text.len() && full_text.as_bytes()[cluster] == b' ' {
                            space_count += 1;
                        }
                    }
                }
            }

            if space_count > 0 && free_space > 0.0 {
                let extra_per_space = free_space / space_count as f32;
                let mut accumulated_offset = 0.0;

                for item in &mut items {
                    item.x += accumulated_offset;

                    if item.end_glyph > 0 {
                        let run = &runs[item.run_index];
                        if item.end_glyph <= run.glyphs.len() {
                            let last_glyph = &run.glyphs[item.end_glyph - 1];
                            let cluster = last_glyph.cluster as usize;
                            if cluster < full_text.len() && full_text.as_bytes()[cluster] == b' ' {
                                accumulated_offset += extra_per_space;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    LineLayout { items, width: max_width, height, baseline }
}

pub fn render_lines(
    ctx: &mut LayoutContext,
    line: &LineLayout,
    shaped_runs: &[ShapedRun],
    y_offset: f32,
    links: &[&str],
    full_text: &str,
) {
    for item in &line.items {
        let run = &shaped_runs[item.run_index];
        render_run_segment(ctx, run, item.start_glyph, item.end_glyph, item.x, y_offset, links, full_text);
    }
}

pub fn render_run_segment(
    ctx: &mut LayoutContext,
    run: &ShapedRun,
    start_glyph: usize,
    end_glyph: usize,
    x: f32,
    y: f32,
    links: &[&str],
    full_text: &str,
) {
    if run.is_image { return; }
    if start_glyph >= run.glyphs.len() { return; }

    let glyphs = &run.glyphs[start_glyph..end_glyph];
    if glyphs.is_empty() { return; }

    let content = &full_text[run.text_range.clone()];

    let href = if run.link_index > 0 && run.link_index <= links.len() {
        Some(links[run.link_index - 1].to_string())
    } else {
        None
    };

    let width: f32 = glyphs.iter().map(|g| g.x_advance).sum();

    let element = PositionedElement {
        x,
        y,
        width,
        height: run.style.text.line_height,
        element: LayoutElement::Text(TextElement {
            content: content.to_string(),
            href,
            text_decoration: run.style.text.text_decoration.clone(),
        }),
        style: run.style.clone(),
    };

    ctx.push_element(element);
}