use crate::core::layout::cache::{MultiSpanCacheKey, ShapingCacheKey};
use crate::core::layout::interface::{LayoutContext, LayoutEnvironment};
use crate::core::layout::{LayoutResult, NodeState, ParagraphState};
use crate::core::layout::text::shaper::{shape_text, ShapedRun};
use crate::core::layout::text::wrapper::{break_lines, render_lines, LineLayout};
use crate::core::layout::LayoutError;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::node::ParagraphNode;

#[derive(Debug, Clone)]
pub struct ParagraphLayout {
    pub lines: Vec<LineLayout>,
    pub total_height: f32,
    pub max_line_width: f32,
    pub shaped_runs: Arc<Vec<ShapedRun>>,
}

impl<'a> ParagraphNode<'a> {
    fn get_shaping_cache_key(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.unique_id.hash(&mut s);
        1u8.hash(&mut s); // Domain 1: Shaping
        s.finish()
    }

    fn get_layout_cache_key(&self, max_width: Option<f32>) -> u64 {
        let mut s = DefaultHasher::new();
        self.unique_id.hash(&mut s);
        2u8.hash(&mut s); // Domain 2: Paragraph Layout
        if let Some(w) = max_width {
            ((w * 100.0).round() as i32).hash(&mut s);
        } else {
            (-1i32).hash(&mut s);
        }
        s.finish()
    }

    /// Resolves shaped runs, checking thread-local generic cache first.
    pub(super) fn resolve_shaping(&self, env: &LayoutEnvironment) -> Arc<Vec<ShapedRun>> {
        let shape_key = self.get_shaping_cache_key();

        if let Some(runs) = env.cache.borrow().get(&shape_key).and_then(|v| v.downcast_ref::<Arc<Vec<ShapedRun>>>()) {
            return runs.clone();
        }

        let runs = self.compute_shaped_runs(env.engine);
        env.cache.borrow_mut().insert(shape_key, Box::new(runs.clone()));
        runs
    }

    fn compute_shaped_runs(&self, engine: &crate::core::layout::LayoutEngine) -> Arc<Vec<ShapedRun>> {
        if self.spans.len() == 1 {
            let span = &self.spans[0];
            let key = ShapingCacheKey {
                text: span.text.to_string(),
                style: span.style.clone(),
            };

            if let Some(runs) = engine.get_cached_shaping_run(&key) {
                return runs;
            }

            let runs = Arc::new(shape_text(engine, self.spans, self.inline_images));
            engine.cache_shaping_run(key, runs.clone());
            return runs;
        }

        if self.inline_images.is_empty() {
            let mut key_spans = Vec::with_capacity(self.spans.len());

            for span in self.spans {
                let mut hasher = DefaultHasher::new();
                span.style.hash(&mut hasher);
                let style_hash = hasher.finish();

                key_spans.push((span.text.to_string(), style_hash));
            }

            let multi_key = MultiSpanCacheKey { spans: key_spans };

            if let Some(runs) = engine.get_cached_multi_span_run(&multi_key) {
                return runs;
            }

            let runs = Arc::new(shape_text(engine, self.spans, self.inline_images));
            engine.cache_multi_span_run(multi_key, runs.clone());
            return runs;
        }

        let runs = shape_text(engine, self.spans, self.inline_images);
        Arc::new(runs)
    }

    pub(super) fn resolve_layout(
        &self,
        env: &LayoutEnvironment,
        shaped_runs: &Arc<Vec<ShapedRun>>,
        width: f32
    ) -> Arc<ParagraphLayout> {
        let layout_key = self.get_layout_cache_key(if width.is_finite() { Some(width) } else { None });

        if let Some(layout) = env.cache.borrow().get(&layout_key).and_then(|v| v.downcast_ref::<Arc<ParagraphLayout>>()) {
            return layout.clone();
        }

        let layout = self.compute_layout(shaped_runs, width);
        env.cache.borrow_mut().insert(layout_key, Box::new(layout.clone()));
        layout
    }

    fn compute_layout(&self, shaped_runs: &Arc<Vec<ShapedRun>>, max_width: f32) -> Arc<ParagraphLayout> {
        let lines = break_lines(shaped_runs, max_width, &self.style, self.full_text);

        let total_height = lines.iter().map(|l| l.height).sum();
        let max_line_width = lines.iter().map(|l| l.width).fold(0.0f32, f32::max);

        Arc::new(ParagraphLayout {
            lines,
            total_height,
            max_line_width,
            shaped_runs: shaped_runs.clone(),
        })
    }

    pub(super) fn render_lines_to_context(
        &self,
        ctx: &mut LayoutContext,
        layout: &ParagraphLayout,
        scroll_offset: f32,
    ) -> Result<LayoutResult, LayoutError> {
        let available_height = ctx.available_height();
        let mut current_y = 0.0;
        let mut start_line_index = 0;

        // Skip lines that were already rendered on previous pages
        while start_line_index < layout.lines.len() {
            if current_y >= scroll_offset - 0.01 {
                break;
            }
            current_y += layout.lines[start_line_index].height;
            start_line_index += 1;
        }

        if start_line_index >= layout.lines.len() {
            ctx.finish_block(self.style.box_model.margin.bottom);
            return Ok(LayoutResult::Finished);
        }

        // Determine how many lines fit on this page
        let mut lines_to_render = 0;
        let mut height_to_render = 0.0;
        let mut split = false;

        for i in start_line_index..layout.lines.len() {
            let line_height = layout.lines[i].height;
            if height_to_render + line_height > available_height + 0.1 {
                split = true;
                break;
            }
            height_to_render += line_height;
            lines_to_render += 1;
        }

        // --- Orphan and Widow Control ---
        let orphans = self.style.misc.orphans;
        let widows = self.style.misc.widows;
        let total_lines = layout.lines.len();
        let lines_remaining = total_lines - (start_line_index + lines_to_render);

        let mut lines_final = lines_to_render;

        // 1. Orphan check
        if start_line_index == 0 && lines_final < orphans && lines_final < total_lines {
            if !ctx.is_at_page_top() {
                return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                    scroll_offset: 0.0,
                })));
            }
        }

        // 2. Widow check
        if split && lines_remaining > 0 && lines_remaining < widows {
            let lines_to_move = widows - lines_remaining;
            if lines_final > lines_to_move {
                lines_final -= lines_to_move;
            } else {
                // Attempt to push everything to the next page.
                // But if we are already at the top of the page (ctx.is_empty()),
                // pushing to the next page won't help (assuming uniform pages).
                // We must make progress to avoid infinite loops.
                if !ctx.is_at_page_top() {
                    lines_final = 0;
                } else {
                    // Force render what we have, violating widow constraints
                    lines_final = lines_to_render;
                }
            }

            if start_line_index == 0 && lines_final < orphans {
                if !ctx.is_at_page_top() {
                    return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                        scroll_offset: 0.0,
                    })));
                }
                lines_final = lines_to_render;
            }
        }

        if lines_final == 0 && split {
            if ctx.is_at_page_top() {
                lines_final = 1;
            } else {
                return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                    scroll_offset: scroll_offset,
                })));
            }
        }

        // --- Rendering ---
        let mut rendered_height_actual = 0.0;
        for i in 0..lines_final {
            let line_idx = start_line_index + i;
            let line = &layout.lines[line_idx];
            render_lines(ctx, line, &layout.shaped_runs, rendered_height_actual, self.links, self.full_text);
            rendered_height_actual += line.height;
        }

        ctx.advance_cursor(rendered_height_actual);

        let next_offset = current_y + rendered_height_actual;

        if start_line_index + lines_final < total_lines {
            Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                scroll_offset: next_offset,
            })))
        } else {
            ctx.finish_block(self.style.box_model.margin.bottom);
            Ok(LayoutResult::Finished)
        }
    }
}