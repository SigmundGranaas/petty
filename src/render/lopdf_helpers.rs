// src/render/lopdf_helpers.rs
//! Standalone helper functions for creating complex PDF structures like
//! annotations and outlines using the `lopdf` library. These are decoupled
// from the main renderer to be reusable by different generation strategies.

use crate::core::layout::{LayoutElement, LayoutEngine, PositionedElement};
use crate::core::style::stylesheet::Stylesheet;
use crate::pipeline::worker::LaidOutSequence;
use crate::render::renderer::{Pass1Result, RenderError};
use crate::render::streaming_writer::StreamingPdfWriter;
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Object, ObjectId, StringFormat};
use std::collections::HashMap;
use std::io::{Seek, Write};
use std::sync::Arc;

/// Creates all link annotations for the document based on the analysis pass results.
///
/// This function iterates through all laid-out elements, finds those with hyperlinks,
/// resolves their destinations using the `pass1_result`, and creates the corresponding
/// `lopdf` annotation dictionaries.
///
/// # Returns
/// A map where the key is the global page index and the value is a vector of
/// `ObjectId`s for the `Link` annotation dictionaries created for that page.
pub fn create_link_annotations<W: Write + Seek>(
    writer: &mut StreamingPdfWriter<W>,
    pass1_result: &Pass1Result,
    all_sequences: &[LaidOutSequence],
    final_page_ids: &[ObjectId],
    page_height: f32,
) -> Result<HashMap<usize, Vec<ObjectId>>, RenderError> {
    let mut link_annots_by_page: HashMap<usize, Vec<ObjectId>> = HashMap::new();
    let mut global_page_idx_offset = 0;

    for seq in all_sequences {
        for (local_page_idx, page_elements) in seq.pages.iter().enumerate() {
            let global_page_idx = global_page_idx_offset + local_page_idx;

            for el in page_elements {
                let href = match &el.element {
                    LayoutElement::Text(t) => t.href.as_ref(),
                    _ => None,
                };

                if let Some(href_str) = href {
                    if let Some(target_id) = href_str.strip_prefix('#') {
                        if let Some(anchor) = pass1_result.resolved_anchors.get(target_id) {
                            if anchor.global_page_index > 0 && anchor.global_page_index <= final_page_ids.len() {
                                let page_annots = link_annots_by_page.entry(global_page_idx).or_default();
                                let target_page_id = final_page_ids[anchor.global_page_index - 1];
                                let y_dest = page_height - anchor.y_pos;

                                let dest = vec![Object::Reference(target_page_id), "FitH".into(), y_dest.into()];
                                let action = dictionary! { "Type" => "Action", "S" => "GoTo", "D" => dest };
                                let action_id = writer.buffer_object(action.into());
                                let rect = vec![
                                    el.x.into(),
                                    (page_height - (el.y + el.height)).into(),
                                    (el.x + el.width).into(),
                                    (page_height - el.y).into(),
                                ];
                                let annot = dictionary! {
                                    "Type" => "Annot", "Subtype" => "Link", "Rect" => rect,
                                    "Border" => vec![0.into(), 0.into(), 0.into()], "A" => action_id,
                                };
                                let annot_id = writer.buffer_object(annot.into());
                                page_annots.push(annot_id);
                            }
                        }
                    }
                }
            }
        }
        global_page_idx_offset += seq.pages.len();
    }
    Ok(link_annots_by_page)
}

/// Creates the PDF document outline (bookmarks) from the Table of Contents entries.
///
/// # Returns
/// The `ObjectId` of the root `Outlines` dictionary if any entries were created, otherwise `None`.
pub fn build_outlines<W: Write + Seek>(
    writer: &mut StreamingPdfWriter<W>,
    pass1_result: &Pass1Result,
    final_page_ids: &[ObjectId],
    page_height: f32,
) -> Result<Option<ObjectId>, RenderError> {
    if pass1_result.toc_entries.is_empty() {
        return Ok(None);
    }

    let toc_entries = &pass1_result.toc_entries;
    let resolved_anchors = &pass1_result.resolved_anchors;

    struct FlatOutlineItem {
        id: ObjectId,
        title: String,
        dest: Vec<Object>,
        parent_idx: Option<usize>,
    }
    struct NodeOutlineItem {
        id: ObjectId,
        title: String,
        dest: Vec<Object>,
        children: Vec<NodeOutlineItem>,
    }

    let mut flat_list = Vec::new();
    let mut level_stack: Vec<(u8, usize)> = vec![(0, usize::MAX)]; // (level, index)

    for entry in toc_entries {
        if let Some(anchor) = resolved_anchors.get(&entry.target_id) {
            if anchor.global_page_index == 0 || anchor.global_page_index > final_page_ids.len() {
                continue;
            }
            let dest_page_id = final_page_ids[anchor.global_page_index - 1];
            let y_dest = page_height - anchor.y_pos;
            let dest = vec![Object::Reference(dest_page_id), "FitH".into(), y_dest.into()];

            while level_stack.last().unwrap().0 >= entry.level {
                level_stack.pop();
            }
            let parent_info = level_stack.last().unwrap();
            let parent_idx = if parent_info.1 == usize::MAX { None } else { Some(parent_info.1) };
            let new_item = FlatOutlineItem {
                id: writer.new_object_id(),
                title: entry.text.clone(),
                dest,
                parent_idx,
            };
            let new_idx = flat_list.len();
            flat_list.push(new_item);
            level_stack.push((entry.level, new_idx));
        }
    }

    if flat_list.is_empty() {
        return Ok(None);
    }
    let mut children_map: HashMap<usize, Vec<NodeOutlineItem>> = HashMap::new();
    let mut root_items = Vec::new();

    for (i, flat_node) in flat_list.into_iter().enumerate().rev() {
        let mut children = children_map.remove(&i).unwrap_or_default();
        children.reverse();
        let node = NodeOutlineItem { id: flat_node.id, title: flat_node.title, dest: flat_node.dest, children };
        if let Some(parent_idx) = flat_node.parent_idx {
            children_map.entry(parent_idx).or_default().push(node);
        } else {
            root_items.push(node);
        }
    }
    root_items.reverse();

    if root_items.is_empty() {
        return Ok(None);
    }

    let outline_root_id = writer.new_object_id();
    let first_id = root_items.first().unwrap().id;
    let last_id = root_items.last().unwrap().id;
    writer.buffer_object_at_id(outline_root_id, dictionary! {
        "Type" => "Outlines", "First" => first_id, "Last" => last_id, "Count" => root_items.len() as i64,
    }.into());

    fn buffer_outline_level<W: Write + Seek>(items: &[NodeOutlineItem], parent_id: ObjectId, writer: &mut StreamingPdfWriter<W>) {
        for (i, item) in items.iter().enumerate() {
            let mut dict = dictionary! {
                "Title" => Object::String(to_win_ansi(&item.title), StringFormat::Literal),
                "Parent" => parent_id, "Dest" => item.dest.clone(),
            };
            if i > 0 { dict.set("Prev", items[i - 1].id); }
            if i < items.len() - 1 { dict.set("Next", items[i + 1].id); }
            if !item.children.is_empty() {
                dict.set("First", item.children.first().unwrap().id);
                dict.set("Last", item.children.last().unwrap().id);
                dict.set("Count", -(item.children.len() as i64));
                buffer_outline_level(&item.children, item.id, writer);
            }
            writer.buffer_object_at_id(item.id, dict.into());
        }
    }
    buffer_outline_level(&root_items, outline_root_id, writer);
    Ok(Some(outline_root_id))
}


/// A simplified version of the page content rendering logic, extracted for reuse.
pub fn render_elements_to_content(
    elements: Vec<PositionedElement>,
    _layout_engine: &LayoutEngine,
    _stylesheet: &Arc<Stylesheet>,
    _page_width: f32,
    page_height: f32,
) -> Result<Content, RenderError> {
    // This function needs font_map, which isn't easily accessible here without
    // re-creating it. For now, we'll create a temporary one.
    // This highlights that the rendering context should probably be an object passed around.
    let mut font_map = HashMap::new();
    // A bit of a hack: can't access layout_engine's font manager directly.
    // Let's assume a default font for now.
    font_map.insert("Helvetica".to_string(), "F1".to_string());

    let mut page_ctx = PageContext::new(page_height, &font_map);
    for el in &elements {
        page_ctx.draw_element(el)?;
    }
    Ok(page_ctx.finish())
}

// --- Internal Page Drawing Context ---

use crate::core::layout::{ComputedStyle, ImageElement, TextElement};
use crate::core::style::color::Color;
use crate::core::style::font::FontWeight;
use once_cell::sync::Lazy;

static DEFAULT_LOPDF_FONT_NAME: Lazy<String> = Lazy::new(|| "F1".to_string());


struct PageContext<'a> {
    page_height: f32,
    content: Content,
    state: LopdfPageRenderState,
    font_map: &'a HashMap<String, String>,
}
#[derive(Default, Clone, PartialEq)]
struct LopdfPageRenderState {
    font_name: String,
    font_size: f32,
    fill_color: Color,
}

impl<'a> PageContext<'a> {
    fn new(page_height: f32, font_map: &'a HashMap<String, String>) -> Self {
        Self { page_height, content: Content { operations: vec![] }, state: Default::default(), font_map }
    }
    fn finish(self) -> Content { self.content }
    fn draw_element(&mut self, el: &PositionedElement) -> Result<(), RenderError> {
        self.draw_background_and_borders(el)?;
        match &el.element {
            LayoutElement::Text(text) => self.draw_text(text, el)?,
            LayoutElement::Image(image) => self.draw_image(image, el)?,
            _ => {}
        }
        Ok(())
    }
    fn draw_background_and_borders(&mut self, el: &PositionedElement) -> Result<(), RenderError> {
        let style = &el.style;
        let x = el.x;
        let y = self.page_height - (el.y + el.height);
        if let Some(bg) = &style.background_color {
            self.content.operations.push(Operation::new("rg", vec![(bg.r as f32 / 255.0).into(), (bg.g as f32 / 255.0).into(), (bg.b as f32 / 255.0).into()]));
            self.content.operations.push(Operation::new("re", vec![x.into(), y.into(), el.width.into(), el.height.into()]));
            self.content.operations.push(Operation::new("f", vec![]));
        }
        if let Some(border) = &style.border_bottom {
            self.content.operations.push(Operation::new("w", vec![border.width.into()]));
            self.content.operations.push(Operation::new("RG", vec![(border.color.r as f32 / 255.0).into(), (border.color.g as f32 / 255.0).into(), (border.color.b as f32 / 255.0).into()]));
            if border.style == crate::core::style::border::BorderStyle::Dotted { self.content.operations.push(Operation::new("d", vec![vec![Object::Integer(1), Object::Integer(2)].into(), 0.into()])); }
            let line_y = self.page_height - el.y - el.height;
            self.content.operations.push(Operation::new("m", vec![el.x.into(), line_y.into()]));
            self.content.operations.push(Operation::new("l", vec![(el.x + el.width).into(), line_y.into()]));
            self.content.operations.push(Operation::new("S", vec![]));
            self.content.operations.push(Operation::new("d", vec![vec![].into(), 0.into()]));
        }
        Ok(())
    }
    fn get_styled_font_name(style: &Arc<ComputedStyle>) -> String {
        let family = &style.font_family;
        match style.font_weight {
            FontWeight::Bold | FontWeight::Black => format!("{}-Bold", family),
            _ => family.to_string(),
        }
    }
    fn set_font(&mut self, style: &Arc<ComputedStyle>) {
        let styled_font_name = Self::get_styled_font_name(style);
        let internal_font_name = self.font_map
            .get(&styled_font_name)
            .or_else(|| self.font_map.get(style.font_family.as_str()))
            .unwrap_or(&DEFAULT_LOPDF_FONT_NAME);

        if self.state.font_name != *internal_font_name || self.state.font_size != style.font_size {
            self.content.operations.push(Operation::new("Tf", vec![Object::Name(internal_font_name.as_bytes().to_vec()), style.font_size.into()]));
            self.state.font_name = internal_font_name.to_string();
            self.state.font_size = style.font_size;
        }
    }
    fn set_fill_color(&mut self, color: &Color) {
        if self.state.fill_color != *color {
            self.content.operations.push(Operation::new("rg", vec![(color.r as f32 / 255.0).into(), (color.g as f32 / 255.0).into(), (color.b as f32 / 255.0).into()]));
            self.state.fill_color = color.clone();
        }
    }
    fn draw_text(&mut self, text: &TextElement, el: &PositionedElement) -> Result<(), RenderError> {
        if text.content.trim().is_empty() { return Ok(()); }
        self.content.operations.push(Operation::new("BT", vec![]));
        self.set_font(&el.style);
        self.set_fill_color(&el.style.color);
        let baseline_y = el.y + el.style.font_size * 0.8;
        let pdf_y = self.page_height - baseline_y;
        self.content.operations.push(Operation::new("Td", vec![el.x.into(), pdf_y.into()]));
        self.content.operations.push(Operation::new("Tj", vec![Object::String(to_win_ansi(&text.content), StringFormat::Literal)]));
        self.content.operations.push(Operation::new("ET", vec![]));
        Ok(())
    }
    fn draw_image(&mut self, image: &ImageElement, _el: &PositionedElement) -> Result<(), RenderError> {
        log::warn!("Images are not supported in the lopdf streaming renderer yet: {}", image.src);
        Ok(())
    }
}
fn to_win_ansi(s: &str) -> Vec<u8> {
    s.chars().map(|c| if c as u32 <= 255 { c as u8 } else { b'?' }).collect()
}