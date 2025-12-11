// src/core/layout/nodes/paragraph.rs

use crate::core::idf::{IRNode, TextStr};
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::engine::{LayoutEngine, LayoutStore, ShapingCacheKey, MultiSpanCacheKey};
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, NodeState, ParagraphState,
};
use super::RenderNode;
use crate::core::layout::nodes::paragraph_utils::{
    break_lines, shape_text, render_lines, ShapedRun, LineLayout
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::text::{TextBuilder, TextSpan, InlineImageEntry};
use crate::core::layout::LayoutError;
use crate::core::style::dimension::Dimension;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub struct ParagraphBuilder;

impl NodeBuilder for ParagraphBuilder {
    fn build<'a>(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        ParagraphNode::build(node, engine, parent_style, store)
    }
}

#[derive(Debug)]
pub struct ParagraphNode<'a> {
    /// Unique identifier for this node instance, used for stable caching.
    unique_id: usize,
    id: Option<TextStr>,
    pub spans: &'a [TextSpan<'a>],
    pub full_text: &'a str,
    pub links: &'a [&'a str],
    pub inline_images: &'a [InlineImageEntry<'a>],
    style: Arc<ComputedStyle>,
}

impl<'a> ParagraphNode<'a> {
    pub fn build(
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
        store: &'a LayoutStore,
    ) -> Result<RenderNode<'a>, LayoutError> {
        let style = engine.compute_style(node.style_sets(), node.style_override(), &parent_style);
        let style = store.canonicalize_style(style);

        let IRNode::Paragraph {
            meta,
            children: inlines,
        } = node
        else {
            return Err(LayoutError::BuilderMismatch("Paragraph", node.kind()));
        };

        let mut builder = TextBuilder::new(engine, store, &style);
        builder.process_inlines(inlines, &style);

        let (full_text, spans, inline_images_vec, links_vec) = builder.finish();

        let mut link_refs = Vec::with_capacity(links_vec.len());
        for link in links_vec {
            link_refs.push(store.alloc_str(&link));
        }
        let links_slice = store.bump.alloc_slice_copy(&link_refs);
        let images_slice = store.bump.alloc_slice_clone(&inline_images_vec);
        let style_ref = store.cache_style(style);

        let unique_id = store.next_node_id();

        let node = store.bump.alloc(Self {
            unique_id,
            id: meta.id.clone(),
            spans,
            full_text,
            links: links_slice,
            inline_images: images_slice,
            style: style_ref,
        });

        Ok(RenderNode::Paragraph(node))
    }

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

    /// Strategy Step 1: Resolve Text Shaping
    /// Uses two-level caching (node-ID based and structural content based) to avoid repeated shaping.
    fn resolve_shaping(&self, engine: &LayoutEngine, cache: &RefCell<HashMap<u64, Box<dyn Any + Send>>>) -> Arc<Vec<ShapedRun>> {
        let shape_key = self.get_shaping_cache_key();

        // 1. Check local node cache
        if let Some(runs) = cache.borrow().get(&shape_key).and_then(|v| v.downcast_ref::<Arc<Vec<ShapedRun>>>()).cloned() {
            return runs;
        }

        // 2. Compute or fetch from global cache
        let runs = self.compute_shaped_runs(engine);
        cache.borrow_mut().insert(shape_key, Box::new(runs.clone()));
        runs
    }

    fn compute_shaped_runs(&self, engine: &LayoutEngine) -> Arc<Vec<ShapedRun>> {
        // Case 1: Single Span Optimization
        if self.spans.len() == 1 {
            let span = &self.spans[0];
            let key = ShapingCacheKey {
                text: span.text.to_string(),
                style: span.style.clone(),
            };

            if let Some(runs) = engine.get_cached_shaping_run(&key) {
                engine.count_hit();
                return runs;
            }

            engine.count_miss();
            let runs = Arc::new(shape_text(engine, self.spans, self.inline_images));
            engine.cache_shaping_run(key, runs.clone());
            return runs;
        }

        // Case 2: Multi-Span Optimization
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
                engine.count_hit();
                return runs;
            }

            engine.count_miss();
            let runs = Arc::new(shape_text(engine, self.spans, self.inline_images));
            engine.cache_multi_span_run(multi_key, runs.clone());
            return runs;
        }

        // Fallback
        let runs = shape_text(engine, self.spans, self.inline_images);
        Arc::new(runs)
    }

    /// Strategy Step 2: Resolve Line Breaking
    /// Breaks lines based on the shaped runs and the available width.
    fn resolve_layout(
        &self,
        engine: &LayoutEngine,
        cache: &RefCell<HashMap<u64, Box<dyn Any + Send>>>,
        shaped_runs: &Arc<Vec<ShapedRun>>,
        width: f32
    ) -> Arc<ParagraphLayout> {
        let layout_key = self.get_layout_cache_key(if width.is_finite() { Some(width) } else { None });

        if let Some(layout) = cache.borrow().get(&layout_key).and_then(|v| v.downcast_ref::<Arc<ParagraphLayout>>()).cloned() {
            return layout;
        }

        let layout = self.compute_layout(engine, shaped_runs, width);
        cache.borrow_mut().insert(layout_key, Box::new(layout.clone()));
        layout
    }

    fn compute_layout(&self, _engine: &LayoutEngine, shaped_runs: &Arc<Vec<ShapedRun>>, max_width: f32) -> Arc<ParagraphLayout> {
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

    /// Strategy Step 3: Render lines to context
    /// Handles the actual pagination loop, checking vertical space and issuing draw commands.
    fn render_lines_to_context(
        &self,
        ctx: &mut LayoutContext,
        layout: &ParagraphLayout,
        scroll_offset: f32,
    ) -> Result<LayoutResult, LayoutError> {
        let available_height = ctx.available_height();
        let mut current_y = 0.0;
        let mut start_line_index = 0;

        // Fast-forward to resume point
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

        let mut rendered_height = 0.0;
        let mut lines_rendered = 0;
        let mut next_break_offset = 0.0;
        let mut split = false;

        for i in start_line_index..layout.lines.len() {
            let line = &layout.lines[i];

            if rendered_height + line.height > available_height + 0.1 {
                split = true;
                next_break_offset = current_y;
                break;
            }

            render_lines(ctx, line, &layout.shaped_runs, rendered_height, self.links, self.full_text);

            rendered_height += line.height;
            current_y += line.height;
            lines_rendered += 1;
        }

        ctx.advance_cursor(rendered_height);

        if split {
            if lines_rendered == 0 {
                // If we couldn't fit a single line, but we are at the start of the block,
                // we must force a break to avoid infinite loops, OR if we are at top of page, we clip.
                // Here we assume we break and resume on next page.
                return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                    scroll_offset,
                })));
            }
            Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                scroll_offset: next_break_offset,
            })))
        } else {
            ctx.finish_block(self.style.box_model.margin.bottom);
            Ok(LayoutResult::Finished)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParagraphLayout {
    pub lines: Vec<LineLayout>,
    pub total_height: f32,
    pub max_line_width: f32,
    pub shaped_runs: Arc<Vec<ShapedRun>>,
}

impl<'a> LayoutNode for ParagraphNode<'a> {
    fn style(&self) -> &ComputedStyle {
        self.style.as_ref()
    }

    fn measure(&self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Result<Size, LayoutError> {
        let max_width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            f32::INFINITY
        };

        let shaped_runs = self.resolve_shaping(env.engine, env.cache);
        let layout = self.resolve_layout(env.engine, env.cache, &shaped_runs, max_width);

        Ok(self.resolve_size(&layout, constraints))
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

        // Handle pre-layout vertical spacing
        if !is_continuation {
            if ctx.prepare_for_block(self.style.box_model.margin.top) {
                return Ok(LayoutResult::Break(NodeState::Paragraph(ParagraphState {
                    scroll_offset: 0.0,
                })));
            }
        } else {
            ctx.last_v_margin = 0.0;
        }

        let width = if constraints.has_bounded_width() {
            constraints.max_width
        } else {
            ctx.bounds().width
        };

        // Phase 1: Shaping
        let shaped_runs = self.resolve_shaping(ctx.env.engine, ctx.env.cache);

        // Phase 2: Layout
        let layout = self.resolve_layout(ctx.env.engine, ctx.env.cache, &shaped_runs, width);

        // Phase 3: Render
        self.render_lines_to_context(ctx, &layout, scroll_offset)
    }
}

impl<'a> ParagraphNode<'a> {
    fn resolve_size(&self, layout: &ParagraphLayout, constraints: BoxConstraints) -> Size {
        let mut width = layout.max_line_width;
        let mut height = layout.total_height;

        if let Some(Dimension::Pt(w)) = self.style.box_model.width {
            width = w;
        } else if constraints.is_tight() {
            width = constraints.max_width;
        }

        if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            height = h;
        }

        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;
        Size::new(width, height + margin_y)
    }
}