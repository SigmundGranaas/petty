use crate::core::idf::IRNode;
use crate::core::layout::builder::NodeBuilder;
use crate::core::layout::geom::{BoxConstraints, Size};
use crate::core::layout::node::{
    AnchorLocation, LayoutContext, LayoutEnvironment, LayoutNode, LayoutResult, RenderNode,
};
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::text::TextBuilder;
use crate::core::layout::{LayoutEngine, LayoutError, PositionedElement, TextElement};
use crate::core::style::dimension::Dimension;
use crate::core::style::text::TextAlign;
use cosmic_text::{Buffer, LayoutRun, Metrics, Wrap};
use std::sync::{Arc, Mutex};

pub struct ParagraphBuilder;

impl NodeBuilder for ParagraphBuilder {
    fn build(
        &self,
        node: &IRNode,
        engine: &LayoutEngine,
        parent_style: Arc<ComputedStyle>,
    ) -> Result<RenderNode, LayoutError> {
        Ok(RenderNode::Paragraph(ParagraphNode::new(node, engine, parent_style)?))
    }
}

#[derive(Clone)]
pub struct ParagraphNode {
    id: Option<String>,
    text_content: String,
    attrs_list: cosmic_text::AttrsList,
    links: Vec<String>,
    style: Arc<ComputedStyle>,
    scroll_offset: f32,
    buffer: Arc<Mutex<Buffer>>,
}

impl std::fmt::Debug for ParagraphNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParagraphNode")
            .field("id", &self.id)
            .field("text_content", &self.text_content)
            .field("scroll_offset", &self.scroll_offset)
            .finish()
    }
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

        let mut system = engine.font_manager.system.lock().unwrap();
        let metrics = Metrics::new(style.text.font_size, style.text.line_height);
        let mut buffer = Buffer::new(&mut system, metrics);

        let default_attrs = engine.font_manager.attrs_from_style(&style);
        let spans = builder.attrs_list.spans().into_iter().map(|(range, attrs)| {
            (
                &builder.content[range.clone()],
                attrs.as_attrs()
            )
        });

        buffer.set_rich_text(
            &mut system,
            spans,
            &default_attrs,
            cosmic_text::Shaping::Advanced,
            None
        );
        buffer.set_wrap(&mut system, Wrap::Word);

        Ok(Self {
            id: meta.id.clone(),
            text_content: builder.content,
            attrs_list: builder.attrs_list,
            links: builder.links,
            style,
            scroll_offset: 0.0,
            buffer: Arc::new(Mutex::new(buffer)),
        })
    }

    pub fn prepend_text(&mut self, text: &str, engine: &LayoutEngine) {
        let shift_amount = text.len();
        let mut new_content = String::from(text);
        new_content.push_str(&self.text_content);

        let default_attrs = engine.font_manager.attrs_from_style(&self.style);
        let mut new_attrs = cosmic_text::AttrsList::new(&default_attrs);

        new_attrs.add_span(0..shift_amount, &default_attrs);

        for (range, attrs) in self.attrs_list.spans() {
            let new_range = (range.start + shift_amount)..(range.end + shift_amount);
            new_attrs.add_span(new_range, &attrs.as_attrs());
        }

        self.text_content = new_content;
        self.attrs_list = new_attrs;

        let mut buffer = self.buffer.lock().unwrap();
        let mut system = engine.font_manager.system.lock().unwrap();

        let spans = self.attrs_list.spans().into_iter().map(|(range, attrs)| {
            (
                &self.text_content[range.clone()],
                attrs.as_attrs()
            )
        });

        buffer.set_rich_text(
            &mut system,
            spans,
            &default_attrs,
            cosmic_text::Shaping::Advanced,
            None
        );
    }
}

impl LayoutNode for ParagraphNode {
    fn style(&self) -> &Arc<ComputedStyle> {
        &self.style
    }

    fn measure(&mut self, env: &LayoutEnvironment, constraints: BoxConstraints) -> Size {
        let margin_y = self.style.box_model.margin.top + self.style.box_model.margin.bottom;

        let mut buffer = self.buffer.lock().unwrap();
        let mut system = env.engine.font_manager.system.lock().unwrap();

        let set_width = if constraints.has_bounded_width() {
            Some(constraints.max_width)
        } else {
            None
        };

        buffer.set_size(&mut system, set_width, None);
        buffer.shape_until_scroll(&mut system, false);

        let mut max_width: f32 = 0.0;
        let mut height: f32 = 0.0;

        let ascent_correction = buffer.layout_runs().next().map(|r| r.line_y).unwrap_or(0.0);

        for run in buffer.layout_runs() {
            max_width = max_width.max(run.line_w);
            let line_top = run.line_y - ascent_correction;
            height = line_top + run.line_height;
        }

        if let Some(Dimension::Pt(h)) = self.style.box_model.height {
            height = h;
        }

        if self.text_content.is_empty() {
            height = 0.0;
            if let Some(Dimension::Pt(h)) = self.style.box_model.height {
                height = h;
            } else if let Dimension::Pt(h) = self.style.box_model.min_height {
                height = h;
            }
        }

        Size::new(max_width, height + margin_y)
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutContext,
    ) -> Result<LayoutResult, LayoutError> {
        if let Some(id) = &self.id {
            let location = AnchorLocation {
                local_page_index: ctx.local_page_index,
                y_pos: ctx.cursor.1 + ctx.bounds.y,
            };
            ctx.defined_anchors.insert(id.clone(), location);
        }

        let margin_to_add = self.style.box_model.margin.top.max(ctx.last_v_margin);
        let is_continuation = self.scroll_offset > 0.0;

        if !is_continuation {
            if ctx.cursor.1 > 0.0 && margin_to_add > ctx.available_height() {
                return Ok(LayoutResult::Partial(RenderNode::Paragraph(self.clone())));
            }
            ctx.advance_cursor(margin_to_add);
        }
        ctx.last_v_margin = 0.0;

        let mut buffer = self.buffer.lock().unwrap();
        let mut system = ctx.engine.font_manager.system.lock().unwrap();

        let align = match self.style.text.text_align {
            TextAlign::Left => cosmic_text::Align::Left,
            TextAlign::Right => cosmic_text::Align::Right,
            TextAlign::Center => cosmic_text::Align::Center,
            TextAlign::Justify => cosmic_text::Align::Justified,
        };
        for line in buffer.lines.iter_mut() {
            line.set_align(Some(align));
        }

        let width = ctx.bounds.width;
        buffer.set_size(&mut system, Some(width), None);
        buffer.shape_until_scroll(&mut system, false);

        let available_height = ctx.available_height();

        let all_runs: Vec<LayoutRun> = buffer.layout_runs().collect();
        let ascent_correction = all_runs.first().map(|r| r.line_y).unwrap_or(0.0);

        let remaining_runs: Vec<&LayoutRun> = all_runs.iter()
            .filter(|run| (run.line_y - ascent_correction) >= self.scroll_offset - 0.01)
            .collect();

        if remaining_runs.is_empty() {
            ctx.last_v_margin = self.style.box_model.margin.bottom;
            return Ok(LayoutResult::Full);
        }

        let mut runs_to_render = Vec::new();
        let mut next_page_start_y = None;

        let orphans = self.style.misc.orphans.max(1) as usize;
        let widows = self.style.misc.widows.max(1) as usize;

        let mut fit_count = 0;
        for run in &remaining_runs {
            let line_top = run.line_y - ascent_correction;
            let local_y = line_top - self.scroll_offset;

            if local_y + run.line_height <= available_height + 0.1 {
                fit_count += 1;
            } else {
                break;
            }
        }

        if fit_count == remaining_runs.len() {
            runs_to_render.extend(remaining_runs);
        } else {
            if fit_count < orphans {
                if ctx.cursor.1 > 0.0 {
                    return Ok(LayoutResult::Partial(RenderNode::Paragraph(self.clone())));
                }
            }

            let remaining_count = remaining_runs.len() - fit_count;
            if remaining_count < widows {
                let needed = widows - remaining_count;
                if fit_count > needed {
                    fit_count -= needed;
                } else {
                    if ctx.cursor.1 > 0.0 {
                        return Ok(LayoutResult::Partial(RenderNode::Paragraph(self.clone())));
                    }
                }
            }

            if fit_count == 0 && ctx.cursor.1 == 0.0 && !remaining_runs.is_empty() {
                fit_count = 1;
            }

            for i in 0..fit_count {
                runs_to_render.push(remaining_runs[i]);
            }

            if fit_count < remaining_runs.len() {
                let next_run = remaining_runs[fit_count];
                next_page_start_y = Some(next_run.line_y - ascent_correction);
            }
        }

        let mut last_run_bottom = 0.0;
        let is_justified = self.style.text.text_align == TextAlign::Justify;

        for run in runs_to_render {
            let line_top = run.line_y - ascent_correction;
            let local_y = line_top - self.scroll_offset;

            let mut group_glyphs: Vec<&cosmic_text::LayoutGlyph> = Vec::new();
            let mut current_metadata = if let Some(first) = run.glyphs.first() { first.metadata } else { 0 };

            for glyph in run.glyphs.iter() {
                let metadata_changed = glyph.metadata != current_metadata;
                let mut should_break = metadata_changed;

                if !should_break && is_justified && !group_glyphs.is_empty() {
                    let first_g = group_glyphs[0];
                    let group_is_space = run.text[first_g.start..first_g.end].chars().all(char::is_whitespace);
                    let current_is_space = run.text[glyph.start..glyph.end].chars().all(char::is_whitespace);

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
                        run.text
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
                    run.text
                );
            }

            last_run_bottom = local_y + run.line_height;
        }

        if let Some(break_y) = next_page_start_y {
            ctx.advance_cursor(last_run_bottom);
            let mut remainder = self.clone();
            remainder.scroll_offset = break_y;
            return Ok(LayoutResult::Partial(RenderNode::Paragraph(remainder)));
        }

        ctx.advance_cursor(last_run_bottom);
        ctx.last_v_margin = self.style.box_model.margin.bottom;

        Ok(LayoutResult::Full)
    }
}

fn flush_group(
    ctx: &mut LayoutContext,
    glyphs: &[&cosmic_text::LayoutGlyph],
    metadata: usize,
    y: f32,
    height: f32,
    style: &Arc<ComputedStyle>,
    links: &[String],
    full_text: &str
) {
    if glyphs.is_empty() { return; }

    let start_x = glyphs.first().unwrap().x;
    let end_x = glyphs.last().unwrap().x + glyphs.last().unwrap().w;
    let width = end_x - start_x;

    let start_idx = glyphs.first().unwrap().start;
    let end_idx = glyphs.last().unwrap().end;

    let start_idx = start_idx.min(full_text.len());
    let end_idx = end_idx.min(full_text.len());

    let text_segment = &full_text[start_idx..end_idx];

    let href = if metadata > 0 && metadata <= links.len() {
        Some(links[metadata - 1].clone())
    } else {
        None
    };

    let element = PositionedElement {
        x: start_x,
        y,
        width,
        height,
        element: crate::core::layout::LayoutElement::Text(TextElement {
            content: text_segment.to_string(),
            href,
            text_decoration: style.text.text_decoration.clone(),
        }),
        style: style.clone(),
    };
    ctx.push_element(element);
}