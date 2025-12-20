use super::shaper::ShapedRun;
use crate::{ComputedStyle, LayoutContext, LayoutElement, PositionedElement, TextElement};
use petty_style::text::TextAlign;

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

pub fn break_lines(
    runs: &[ShapedRun],
    max_width: f32,
    block_style: &ComputedStyle,
    full_text: &str,
) -> Vec<LineLayout> {
    let mut lines = Vec::new();
    let mut current_line_items = Vec::new();
    let mut current_line_width = 0.0;
    let mut current_line_height = 0.0f32;
    let mut current_line_baseline = 0.0f32;

    for (run_idx, run) in runs.iter().enumerate() {
        if run.is_image {
            if current_line_width + run.width > max_width && !current_line_items.is_empty() {
                lines.push(finalize_line(
                    current_line_items,
                    current_line_width,
                    current_line_height,
                    current_line_baseline,
                    max_width,
                    &block_style.text.text_align,
                    full_text,
                    runs,
                ));
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
                if glyph_idx > glyph_start {
                    current_line_items.push(LineItem {
                        run_index: run_idx,
                        start_glyph: glyph_start,
                        end_glyph: glyph_idx,
                        x: current_line_width,
                        width: current_segment_width,
                    });
                }

                lines.push(finalize_line(
                    current_line_items,
                    current_line_width + current_segment_width,
                    current_line_height,
                    current_line_baseline,
                    max_width,
                    &block_style.text.text_align,
                    full_text,
                    runs,
                ));

                current_line_items = Vec::new();
                current_line_width = 0.0;
                current_segment_width = 0.0;
                current_line_height = run.line_height;
                current_line_baseline = run.baseline_offset;

                glyph_start = glyph_idx + 1;
                glyph_idx += 1;
                continue;
            }

            if is_space {
                if current_line_width + current_segment_width + char_width > max_width
                    && !current_line_items.is_empty()
                {
                    lines.push(finalize_line(
                        current_line_items,
                        current_line_width + current_segment_width,
                        current_line_height,
                        current_line_baseline,
                        max_width,
                        &block_style.text.text_align,
                        full_text,
                        runs,
                    ));
                    current_line_items = Vec::new();
                    current_line_width = 0.0;
                    current_line_height = run.line_height;
                    current_line_baseline = run.baseline_offset;
                    current_segment_width = 0.0;
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
                if !current_line_items.is_empty() {
                    lines.push(finalize_line(
                        current_line_items,
                        current_line_width,
                        current_line_height,
                        current_line_baseline,
                        max_width,
                        &block_style.text.text_align,
                        full_text,
                        runs,
                    ));

                    current_line_items = Vec::new();
                    current_line_width = 0.0;
                    current_line_height = run.line_height;
                    current_line_baseline = run.baseline_offset;
                } else {
                    if glyph_idx > glyph_start {
                        current_line_items.push(LineItem {
                            run_index: run_idx,
                            start_glyph: glyph_start,
                            end_glyph: glyph_idx,
                            x: current_line_width,
                            width: current_segment_width,
                        });
                    }

                    lines.push(finalize_line(
                        current_line_items,
                        current_line_width + current_segment_width,
                        current_line_height,
                        current_line_baseline,
                        max_width,
                        &block_style.text.text_align,
                        full_text,
                        runs,
                    ));

                    current_line_items = Vec::new();
                    current_line_width = 0.0;
                    current_line_height = run.line_height;
                    current_line_baseline = run.baseline_offset;
                    current_segment_width = 0.0;
                    glyph_start = glyph_idx;
                }
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
        lines.push(finalize_line(
            current_line_items,
            current_line_width,
            current_line_height,
            current_line_baseline,
            max_width,
            &block_style.text.text_align,
            full_text,
            runs,
        ));
    }
    lines
}

#[allow(clippy::too_many_arguments)]
fn finalize_line(
    mut items: Vec<LineItem>,
    content_width: f32,
    height: f32,
    baseline: f32,
    max_width: f32,
    align: &TextAlign,
    full_text: &str,
    runs: &[ShapedRun],
) -> LineLayout {
    if !matches!(align, TextAlign::Justify) && !items.is_empty() {
        let mut merged = Vec::with_capacity(items.len());
        let mut current = items[0].clone();

        for next in items.iter().skip(1) {
            if next.run_index == current.run_index
                && next.start_glyph == current.end_glyph
                && (next.x - (current.x + current.width)).abs() < 0.05
            {
                current.end_glyph = next.end_glyph;
                current.width += next.width;
            } else {
                merged.push(current);
                current = next.clone();
            }
        }
        merged.push(current);
        items = merged;
    }

    if !max_width.is_finite() {
        return LineLayout {
            items,
            width: content_width,
            height,
            baseline,
        };
    }

    let mut effective_width = content_width;

    // For justified text, trim the trailing space from the last item.
    // This serves two purposes:
    // 1. It makes `effective_width` accurate (visual width of text).
    // 2. It simplifies gap counting (we don't count the last space as a gap to fill).
    if matches!(align, TextAlign::Justify)
        && let Some(last) = items.last_mut()
        && last.end_glyph > 0
    {
        let run = &runs[last.run_index];
        if last.end_glyph <= run.glyphs.len() {
            let last_glyph_idx = last.end_glyph - 1;
            let last_glyph = &run.glyphs[last_glyph_idx];
            let cluster = last_glyph.cluster as usize;

            if cluster < full_text.len() && full_text.as_bytes()[cluster] == b' ' {
                // Found trailing space. Trim it.
                effective_width -= last_glyph.x_advance;
                last.width -= last_glyph.x_advance;
                last.end_glyph -= 1;
            }
        }
    }

    let free_space = (max_width - effective_width).max(0.0);

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
    LineLayout {
        items,
        width: max_width,
        height,
        baseline,
    }
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
        render_run_segment(
            ctx,
            run,
            item.start_glyph,
            item.end_glyph,
            item.x,
            y_offset,
            links,
            full_text,
        );
    }
}

#[allow(clippy::too_many_arguments)]
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
    if run.is_image {
        return;
    }
    if start_glyph >= run.glyphs.len() {
        return;
    }

    let actual_end_glyph = end_glyph.min(run.glyphs.len());
    if start_glyph >= actual_end_glyph {
        return;
    }

    let glyphs = &run.glyphs[start_glyph..actual_end_glyph];
    if glyphs.is_empty() {
        return;
    }

    let start_cluster = glyphs[0].cluster as usize;
    let end_cluster = if actual_end_glyph < run.glyphs.len() {
        run.glyphs[actual_end_glyph].cluster as usize
    } else {
        run.text_range.end
    };

    let (byte_start, byte_end) = if start_cluster <= end_cluster {
        (start_cluster, end_cluster)
    } else {
        (end_cluster, start_cluster)
    };

    let safe_start = byte_start.max(run.text_range.start).min(run.text_range.end);
    let safe_end = byte_end.max(run.text_range.start).min(run.text_range.end);

    let content = &full_text[safe_start..safe_end];

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
