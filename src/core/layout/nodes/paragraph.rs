use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::nodes::paragraph_utils::flush_group;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::text::TextBuilder;
use crate::core::layout::nodes::image::ImageNode;
use crate::core::layout::{LayoutEngine, LayoutError};
use crate::core::style::dimension::Dimension;
use crate::core::style::text::TextAlign;
use cosmic_text::{AttrsList, Buffer, LayoutRun, Metrics, Wrap};
use std::sync::{Arc, Mutex};
use std::any::Any;
use std::time::Instant;
use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;

pub struct ParagraphBuilder;

impl NodeBuilder for ParagraphBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(Box::new(ParagraphNode::new(node, engine, parent_style)?))
    }
}

// Increased cache size to handle oscillation between infinite measure and fixed width layout
const CACHE_SIZE: usize = 4;

#[derive(Debug)]
pub struct ParagraphNode {
    _id: Option<String>,
    _text_content: String,
    attrs_list: AttrsList,
    links: Vec<String>,
    _inline_images: Vec<(usize, ImageNode)>,
    style: Arc<ComputedStyle>,
    // Wrapped in Option to delay allocation until first measure/layout
    buffer: Mutex<Option<Buffer>>,
    // Cache keyed by wrapping width (Option<f32>) -> Resulting Size
    measure_cache: Arc<Mutex<Vec<(Option<f32>, Size)>>>,
    // Caches the width used for the last shape operation to prevent redundant shaping
    last_shaped_width: Mutex<Option<f32>>,
}

#[derive(Debug)]
struct ParagraphState {
    scroll_offset: f32,
}

impl ParagraphNode {
    pub fn new(node: &IRNode, engine: &LayoutEngine, parent_style: Arc<ComputedStyle>) -> Result<Self, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let (meta, inlines) = match node {
            IRNode::Paragraph { meta, children } => (meta, children),
            _ => return Err(LayoutError::BuilderMismatch("Paragraph", node.kind())),
        };

        let mut builder = TextBuilder::new(engine, &style);
        builder.process_inlines(inlines, &style);

        Ok(Self {
            _id: meta.id.clone(),
            _text_content: builder.content,
            attrs_list: builder.attrs_list,
            links: builder.links,
            _inline_images: builder.inline_images,
            style,
            buffer: Mutex::new(None),
            measure_cache: Arc::new(Mutex::new(Vec::with_capacity(CACHE_SIZE))),
            last_shaped_width: Mutex::new(None),
        })
    }

    /// Helper to initialize the buffer and set text.
    /// Requires the system lock, so caller must only call this if guard is None.
    fn initialize_buffer(
        &self,
        system: &mut cosmic_text::FontSystem,
        font_manager: &crate::core::layout::FontManager,
    ) -> Buffer {
        let metrics = Metrics::new(self.style.text.font_size, self.style.text.line_height);
        let mut buffer = Buffer::new(system, metrics);

        buffer.set_wrap(system, Wrap::Word);

        let align = match self.style.text.text_align {
            TextAlign::Left => cosmic_text::Align::Left,
            TextAlign::Right => cosmic_text::Align::Right,
            TextAlign::Center => cosmic_text::Align::Center,
            TextAlign::Justify => cosmic_text::Align::Justified,
        };
        for line in buffer.lines.iter_mut() {
            line.set_align(Some(align));
        }

        let default_attrs = font_manager.attrs_from_style(&self.style);
        let spans = self.attrs_list.spans().into_iter().map(|(range, attrs)| {
            (
                &self._text_content[range.clone()],
                attrs.as_attrs()
            )
        });

        buffer.set_rich_text(
            system,
            spans,
            &default_attrs,
            cosmic_text::Shaping::Advanced,
            None
        );

        buffer
    }
}

impl LayoutNode for ParagraphNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let set_width = if constraints.has_bounded_width() {
            Some(constraints.max_width)
        } else {
            None
        };

        // 1. Check Cache
        if let Ok(cache) = self.measure_cache.lock() {
            for (cached_width, cached_size) in cache.iter() {
                if width_fuzzy_eq(set_width, *cached_width) {
                    return *cached_size;
                }
                // Reuse unbounded calculation if it fits within constraints
                if cached_width.is_none() {
                    if let Some(target_w) = set_width {
                        if target_w >= cached_size.width - 0.01 {
                            return *cached_size;
                        }
                    }
                }
            }
        }

        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        // 2. Prepare Buffer (Lazy Init)
        let mut buffer_guard = self.buffer.lock().unwrap();

        // Critical Optimization: Check if init is needed BEFORE locking the global font system
        if buffer_guard.is_none() {
            let start = Instant::now();
            let mut system = env.engine.font_manager.system.lock().unwrap();
            let lock_time = start.elapsed();
            if lock_time.as_millis() > 50 {
                log::warn!("Paragraph measure lock took {}ms", lock_time.as_millis());
            }

            *buffer_guard = Some(self.initialize_buffer(&mut system, &env.engine.font_manager));
        }

        let buffer = buffer_guard.as_mut().unwrap();
        let mut last_width_guard = self.last_shaped_width.lock().unwrap();

        // 3. Determine if Reshape Needed
        let width_changed = match (*last_width_guard, set_width) {
            (Some(curr), Some(target)) => (curr - target).abs() > 0.01,
            (Some(_), None) => true,
            (None, Some(target)) => {
                // Check if current content fits without reshaping
                let mut max_w: f32 = 0.0;
                for run in buffer.layout_runs() {
                    max_w = max_w.max(run.line_w);
                }

                if max_w == 0.0 && !buffer.lines.is_empty() && buffer.layout_runs().count() == 0 {
                    true // Unshaped state
                } else {
                    max_w > target + 0.01
                }
            },
            (None, None) => {
                buffer.layout_runs().count() == 0 && !self._text_content.is_empty()
            },
        };

        if width_changed {
            // Only lock global system if we actually need to shape
            let start = Instant::now();
            let mut system = env.engine.font_manager.system.lock().unwrap();
            let lock_time = start.elapsed();

            buffer.set_size(&mut system, set_width, None);
            buffer.shape_until_scroll(&mut system, false);
            *last_width_guard = set_width;

            let total_time = start.elapsed();
            if total_time.as_millis() > 50 {
                log::warn!("Paragraph measure reshape took {}ms (lock: {}ms)", total_time.as_millis(), lock_time.as_millis());
            }
        }

        // 4. Calculate size from buffer (Fast, local memory)
        let mut max_width: f32 = 0.0;
        let mut height: f32 = 0.0;

        let runs: Vec<_> = buffer.layout_runs().collect();
        let ascent_correction = runs.first().map(|r| r.line_y).unwrap_or(0.0);

        for run in runs {
            max_width = max_width.max(run.line_w);
            let line_top = run.line_y - ascent_correction;
            height = line_top + run.line_height;
        }

        if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            height = h;
        }

        let size = Size::new(max_width, height + margin_y);

        // 5. Update Cache
        if let Ok(mut cache) = self.measure_cache.lock() {
            if cache.len() >= CACHE_SIZE {
                cache.remove(0);
            }
            cache.push((set_width, size));
        }

        size
    }

    fn layout(
        &self,
        ctx: &mut LayoutContext,
        constraints: BoxConstraints,
        break_state: Option<Box<dyn Any + Send>>,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self._id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }

        let scroll_offset = if let Some(state) = break_state {
            match state.downcast::<ParagraphState>() {
                Ok(s) => s.scroll_offset,
                Err(_) => return Err(LayoutError::Generic("Invalid state passed to ParagraphNode".to_string())),
            }
        } else {
            0.0
        };

        let is_continuation = scroll_offset > 0.0;
        let mut margin_applied = 0.0;

        if !is_continuation {
            let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);

            if ctx.cursor.1 > 0.1 && margin_to_add > ctx.available_height() {
                return Ok(LayoutResult::Break(Box::new(ParagraphState { scroll_offset: 0.0 })));
            }

            ctx.advance_cursor(margin_to_add);
            margin_applied = margin_to_add;
        }
        ctx.last_v_margin = 0.0;

        let width = constraints.has_bounded_width().then_some(constraints.max_width).unwrap_or(ctx.bounds.width);

        let mut buffer_guard = self.buffer.lock().unwrap();

        // Lazy Init (Check before lock)
        if buffer_guard.is_none() {
            let mut system = ctx.engine.font_manager.system.lock().unwrap();
            *buffer_guard = Some(self.initialize_buffer(&mut system, &ctx.engine.font_manager));
        }

        let buffer = buffer_guard.as_mut().unwrap();
        let mut last_width_guard = self.last_shaped_width.lock().unwrap();

        let width_changed = match *last_width_guard {
            Some(w) => (w - width).abs() > 0.01,
            None => {
                let mut max_w: f32 = 0.0;
                for run in buffer.layout_runs() {
                    max_w = max_w.max(run.line_w);
                }
                (max_w == 0.0 && !self._text_content.is_empty()) || max_w > width + 0.01
            },
        };

        if width_changed {
            let start = Instant::now();
            let mut system = ctx.engine.font_manager.system.lock().unwrap();
            let lock_time = start.elapsed();

            buffer.set_size(&mut system, Some(width), None);
            buffer.shape_until_scroll(&mut system, false);
            *last_width_guard = Some(width);

            let total_time = start.elapsed();
            if total_time.as_millis() > 50 {
                log::warn!("Paragraph layout reshape took {}ms (lock: {}ms)", total_time.as_millis(), lock_time.as_millis());
            }
        }

        let available_height = ctx.available_height();

        let all_runs: Vec<LayoutRun> = buffer.layout_runs().collect();
        let ascent_correction = all_runs.first().map(|r| r.line_y).unwrap_or(0.0);

        let remaining_runs: Vec<&LayoutRun> = all_runs.iter()
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
        let is_at_top_of_container = ctx.cursor.1 <= at_top_threshold;

        if fit_count < remaining_runs.len() {
            if fit_count < orphans && !is_continuation {
                if !is_at_top_of_container {
                    forced_break_to_start = true;
                }
            }

            if !forced_break_to_start {
                let remaining_after = remaining_runs.len() - fit_count;
                if remaining_after < widows {
                    let needed_on_next = widows - remaining_after;
                    if fit_count > needed_on_next {
                        fit_count -= needed_on_next;
                    } else {
                        if !is_continuation && !is_at_top_of_container {
                            forced_break_to_start = true;
                        }
                    }
                }
            }
        }

        if forced_break_to_start {
            return Ok(LayoutResult::Break(Box::new(ParagraphState { scroll_offset: 0.0 })));
        }

        if fit_count == 0 && !remaining_runs.is_empty() {
            if !is_at_top_of_container {
                return Ok(LayoutResult::Break(Box::new(ParagraphState { scroll_offset })));
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
            let mut current_metadata = if let Some(first) = run.glyphs.first() { first.metadata } else { 0 };

            for glyph in run.glyphs.iter() {
                let metadata_changed = glyph.metadata != current_metadata;
                let mut should_break = metadata_changed;

                if !should_break && is_justified && !group_glyphs.is_empty() {
                    let first_g: &cosmic_text::LayoutGlyph = group_glyphs[0];
                    let group_is_space = run.text[first_g.start..first_g.end].chars().all(char::is_whitespace);
                    let current_is_space = run.text[glyph.start..glyph.end].chars().all(char::is_whitespace);
                    if group_is_space != current_is_space {
                        should_break = true;
                    }
                }

                if should_break {
                    flush_group(ctx, &group_glyphs, current_metadata, local_y, run.line_height, &self.style, &self.links, run.text);
                    group_glyphs.clear();
                    current_metadata = glyph.metadata;
                }
                group_glyphs.push(glyph);
            }
            if !group_glyphs.is_empty() {
                flush_group(ctx, &group_glyphs, current_metadata, local_y, run.line_height, &self.style, &self.links, run.text);
            }

            last_run_bottom = local_y + run.line_height;
        }

        ctx.advance_cursor(last_run_bottom);

        if fit_count < remaining_runs.len() {
            let next_run = remaining_runs[fit_count];
            let next_offset = next_run.line_y - ascent_correction;
            return Ok(LayoutResult::Break(Box::new(ParagraphState { scroll_offset: next_offset })));
        }

        ctx.last_v_margin = self.style.box_model.margin.bottom;
        Ok(LayoutResult::Finished)
    }
}

fn width_fuzzy_eq(a: Option<f32>, b: Option<f32>) -> bool {
    match (a, b) {
        (Some(va), Some(vb)) => (va - vb).abs() < 0.01,
        (None, None) => true,
        _ => false,
    }
}