// src/core/layout/nodes/paragraph.rs

use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, ParagraphState,
    RenderNode,
};
use crate::core::layout::nodes::image::ImageNode;
use crate::core::layout::nodes::paragraph_utils::flush_group;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::text::TextBuilder;
use crate::core::layout::{LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use crate::core::style::text::TextAlign;
use bumpalo::Bump;
use cosmic_text::{AttrsList, Buffer, LayoutRun, Metrics, Wrap};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

pub struct ParagraphBuilder;

impl NodeBuilder for ParagraphBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        ParagraphNode::build(node, engine, parent_style, arena)
    }
}

#[derive(Debug)]
pub struct ParagraphNode<'a> {
    id: Option<TextStr>,
    text_content: String,
    attrs_list: AttrsList,
    links: Vec<String>,
    _inline_images: Vec<(usize, &'a ImageNode)>,
    style: Arc<ComputedStyle>,
}

impl<'a> ParagraphNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        arena: &'a Bump,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);

        let IRNode::Paragraph {
            meta,
            children: inlines,
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("Paragraph", node.kind()));
        };

        // TextBuilder now takes arena
        let mut builder = TextBuilder::new(engine, arena, &style);
        builder.process_inlines(inlines, &style);

        let node = arena.alloc(Self {
            id: meta.id.clone(),
            text_content: builder.content,
            attrs_list: builder.attrs_list,
            links: builder.links,
            _inline_images: builder.inline_images,
            style,
        });

        Ok(RenderNode::Paragraph(node))
    }

    /// Generates a cache key for the paragraph based on its pointer identity and constraints.
    fn get_cache_key(&self, max_width: Option<f32>) -> u64 {
        let mut s = DefaultHasher::new();
        // Hash the pointer address to identify this specific paragraph instance
        (self as *const Self).hash(&mut s);
        // Add content length for extra safety against address reuse collisions
        self.text_content.len().hash(&mut s);
        // Hash the constraint to differentiate layouts at different widths
        if let Some(w) = max_width {
            ((w * 100.0) as i64).hash(&mut s);
        } else {
            (-1i64).hash(&mut s);
        }
        s.finish()
    }

    /// Creates and shapes a buffer for the current content and constraints.
    fn shape_text(&self, engine: &LayoutEngine, max_width: Option<f32>) -> Buffer {
        let start = Instant::now();

        let mut font_system = engine.font_system();

        let metrics = Metrics::new(self.style.text.font_size, self.style.text.line_height);
        let mut buffer = Buffer::new(&mut *font_system, metrics);

        buffer.set_wrap(&mut *font_system, Wrap::Word);
        buffer.set_size(&mut *font_system, max_width, None);

        let default_attrs = engine.attrs_from_style(&self.style);
        let spans = self
            .attrs_list
            .spans()
            .into_iter()
            .map(|(range, attrs)| (&self.text_content[range.clone()], attrs.as_attrs()));

        buffer.set_rich_text(
            &mut *font_system,
            spans,
            &default_attrs,
            cosmic_text::Shaping::Advanced,
            None,
        );

        let align = match self.style.text.text_align {
            TextAlign::Left => cosmic_text::Align::Left,
            TextAlign::Right => cosmic_text::Align::Right,
            TextAlign::Center => cosmic_text::Align::Center,
            TextAlign::Justify => cosmic_text::Align::Justified,
        };

        for line in buffer.lines.iter_mut() {
            line.set_align(Some(align));
        }

        buffer.shape_until_scroll(&mut *font_system, false);

        let duration = start.elapsed();
        engine.record_perf("Paragraph::shape_text", duration);

        buffer
    }
}

impl<'a> LayoutNode for ParagraphNode<'a> {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &mut LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let max_width = if constraints.has_bounded_width() {
            Some(constraints.max_width)
        } else {
            None
        };

        // KEY OPTIMIZATION: Check cache in measure!
        let cache_key = self.get_cache_key(max_width);

        let buffer = if let Some(cached) = env.cache.get(&cache_key) {
            // log::trace!("[PERF] Paragraph cache HIT in measure");
            if let Some(b) = cached.downcast_ref::<Buffer>() {
                b.clone()
            } else {
                let b = self.shape_text(env.engine, max_width);
                env.cache.insert(cache_key, Box::new(b.clone()));
                b
            }
        } else {
            // log::trace!("[PERF] Paragraph cache MISS in measure");
            let b = self.shape_text(env.engine, max_width);
            env.cache.insert(cache_key, Box::new(b.clone()));
            b
        };

        let mut measured_width: f32 = 0.0;
        let mut height: f32 = 0.0;

        let runs: Vec<_> = buffer.layout_runs().collect();
        let ascent_correction = runs.first().map(|r| r.line_y).unwrap_or(0.0);

        for run in runs {
            measured_width = measured_width.max(run.line_w);
            let line_top = run.line_y - ascent_correction;
            height = line_top + run.line_height;
        }

        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            height = h;
        }

        Size::new(measured_width, height + margin_y)
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<NodeState>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            ctx.register_anchor(id);
        }

        let scroll_offset = if let Some(state) = break_state {
            state.as_paragraph()?.scroll_offset
        } else {
            0.0
        };

        let is_continuation = scroll_offset > 0.0;
        let mut margin_applied = 0.0;

        if !is_continuation {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);

            if ctx.cursor_y() > 0.1 && margin_to_add > ctx.available_height() {
                return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                    scroll_offset: 0.0,
                })));
            }

            ctx.advance_cursor(margin_to_add);
            margin_applied = margin_to_add;
        }
        ctx.last_v_margin = 0.0;

        let width = if constraints.has_bounded_width() {
            Some(constraints.max_width)
        } else {
            Some(ctx.bounds().width)
        };

        // Check cache before shaping (cache is in ctx.env)
        let cache_key = self.get_cache_key(width);

        let buffer = if let Some(cached) = ctx.env.cache.get(&cache_key) {
            // log::trace!("[PERF] Paragraph cache HIT in layout");
            if let Some(b) = cached.downcast_ref::<Buffer>() {
                b.clone()
            } else {
                let b = self.shape_text(ctx.env.engine, width);
                ctx.env.cache.insert(cache_key, Box::new(b.clone()));
                b
            }
        } else {
            // log::trace!("[PERF] Paragraph cache MISS in layout");
            let b = self.shape_text(ctx.env.engine, width);
            ctx.env.cache.insert(cache_key, Box::new(b.clone()));
            b
        };

        let available_height = ctx.available_height();

        let all_runs: Vec<LayoutRun> = buffer.layout_runs().collect();
        let ascent_correction = all_runs.first().map(|r| r.line_y).unwrap_or(0.0);

        let remaining_runs: Vec<&LayoutRun> = all_runs
            .iter()
            .filter(|run| (run.line_y - ascent_correction) >= scroll_offset - 0.01)
            .collect();

        if remaining_runs.is_empty() {
            ctx.last_v_margin = self.style.box_model.margin.bottom;
            return Ok(LayoutResult::Finished);
        }

        let orphans = self.style.misc.orphans.max(1) as usize;
        let widows = self.style.misc.widows.max(1) as usize;

        let mut fit_count = 0;
        for run in &remaining_runs {
            let line_top = run.line_y - ascent_correction;
            let local_y = line_top - scroll_offset;
            if local_y + run.line_height <= available_height + 0.1 {
                fit_count += 1;
            } else {
                break;
            }
        }

        let mut forced_break_to_start = false;
        let at_top_threshold = if is_continuation { 0.1 } else { margin_applied + 2.0 };
        let is_at_top_of_container = ctx.cursor_y() <= at_top_threshold;

        if fit_count < remaining_runs.len() {
            if fit_count < orphans && !is_continuation && !is_at_top_of_container {
                forced_break_to_start = true;
            }
            if !forced_break_to_start {
                let remaining_after = remaining_runs.len() - fit_count;
                if remaining_after < widows {
                    let needed_on_next = widows - remaining_after;
                    if fit_count > needed_on_next {
                        fit_count -= needed_on_next;
                    } else if !is_continuation && !is_at_top_of_container {
                        forced_break_to_start = true;
                    }
                }
            }
        }

        if forced_break_to_start {
            return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                scroll_offset: 0.0,
            })));
        }

        if fit_count == 0 && !remaining_runs.is_empty() {
            if !is_at_top_of_container {
                return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                    scroll_offset,
                })));
            }
            fit_count = 1;
        }

        let mut last_run_bottom = 0.0;
        let is_justified = self.style.text.text_align == TextAlign::Justify;

        for i in 0..fit_count {
            let run = remaining_runs[i];
            let line_top = run.line_y - ascent_correction;
            let local_y = line_top - scroll_offset;

            let mut group_glyphs = Vec::new();
            let mut current_metadata = run.glyphs.first().map(|g| g.metadata).unwrap_or(0);

            for glyph in run.glyphs.iter() {
                let metadata_changed = glyph.metadata != current_metadata;
                let mut should_break = metadata_changed;

                if !should_break && is_justified && !group_glyphs.is_empty() {
                    let first_g: &cosmic_text::LayoutGlyph = group_glyphs[0];
                    let group_is_space = run.text[first_g.start..first_g.end]
                        .chars()
                        .all(char::is_whitespace);
                    let current_is_space = run.text[glyph.start..glyph.end]
                        .chars()
                        .all(char::is_whitespace);
                    if group_is_space != current_is_space {
                        should_break = true;
                    }
                }

                if should_break {
                    flush_group(
                        ctx,
                        &group_glyphs,
                        current_metadata,
                        local_y,
                        run.line_height,
                        &self.style,
                        &self.links,
                        run.text,
                    );
                    group_glyphs.clear();
                    current_metadata = glyph.metadata;
                }
                group_glyphs.push(glyph);
            }
            if !group_glyphs.is_empty() {
                flush_group(
                    ctx,
                    &group_glyphs,
                    current_metadata,
                    local_y,
                    run.line_height,
                    &self.style,
                    &self.links,
                    run.text,
                );
            }

            last_run_bottom = local_y + run.line_height;
        }

        ctx.advance_cursor(last_run_bottom);

        if fit_count < remaining_runs.len() {
            let next_run = remaining_runs[fit_count];
            let next_offset = next_run.line_y - ascent_correction;
            Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                scroll_offset: next_offset,
            })))
        } else {
            ctx.last_v_margin = self.style.box_model.margin.bottom;
            Ok(LayoutResult::Finished)
        }
    }
}