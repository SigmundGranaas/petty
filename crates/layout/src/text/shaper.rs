use crate::LayoutEngine;
use crate::interface::LayoutNode;
use crate::style::ComputedStyle;
use crate::text::builder::{InlineImageEntry, TextSpan};
use rustybuzz::{Feature, UnicodeBuffer};
use std::cell::RefCell;
use std::sync::Arc;
use ttf_parser::Tag;

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
    pub font_data: Option<crate::fonts::FontData>,
    pub font_size: f32,
    pub link_index: usize,
    pub is_image: bool,
    pub text_range: std::ops::Range<usize>,
    pub ascender: f32,
    pub line_height: f32,
    pub baseline_offset: f32,
}

pub fn shape_text(
    engine: &LayoutEngine,
    spans: &[TextSpan],
    images: &[InlineImageEntry],
) -> Vec<ShapedRun> {
    let mut runs = Vec::new();
    let mut current_char_idx = 0;

    static FEATURES: std::sync::OnceLock<Vec<Feature>> = std::sync::OnceLock::new();
    let features = FEATURES.get_or_init(|| {
        vec![
            Feature::new(Tag::from_bytes(b"liga"), 1, ..),
            Feature::new(Tag::from_bytes(b"kern"), 1, ..),
        ]
    });

    let mut last_style_ref: Option<&Arc<ComputedStyle>> = None;
    let mut last_font_data: Option<crate::fonts::FontData> = None;

    for span in spans {
        let span_len = span.text.len();

        if let Some(img_entry) = images.iter().find(|img| img.index == current_char_idx) {
            let width = if let Some(petty_style::dimension::Dimension::Pt(w)) =
                img_entry.node.style().box_model.width
            {
                w
            } else {
                0.0
            };
            let height = if let Some(petty_style::dimension::Dimension::Pt(h)) =
                img_entry.node.style().box_model.height
            {
                h
            } else {
                0.0
            };

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
                let fd = engine
                    .get_font_for_style(&span.style)
                    .or_else(|| engine.get_font_for_style(&engine.get_default_style()));
                last_style_ref = Some(&span.style);
                last_font_data = fd.clone();
                fd
            }
        } else {
            let fd = engine
                .get_font_for_style(&span.style)
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

        let mut buffer =
            SCRATCH_BUFFER.with(|b| b.borrow_mut().take().unwrap_or_else(UnicodeBuffer::new));
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
