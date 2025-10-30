// src/render/lopdf_renderer.rs
use super::renderer::{self, DocumentRenderer, ResolvedAnchor};
use super::streaming_writer::StreamingPdfWriter;
use crate::core::idf::{IRNode, InlineNode, NodeMetadata, SharedData};
use crate::core::layout::{
    ComputedStyle, ImageElement, LayoutElement, LayoutEngine, PositionedElement, TextElement,
};
use crate::core::style::color::Color;
use crate::core::style::dimension::{Dimension, Margins};
use crate::core::style::font::FontWeight;
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use crate::pipeline::worker::{LaidOutSequence, TocEntry};
use crate::render::RenderError;
use handlebars::Handlebars;
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Object, ObjectId, StringFormat};
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Cursor, Seek, Write};
use std::sync::Arc;

static DEFAULT_LOPDF_FONT_NAME: Lazy<String> = Lazy::new(|| "F1".to_string());

pub struct LopdfDocumentRenderer<W: Write + Send> {
    final_writer: Option<W>,
    buffer_writer: Option<StreamingPdfWriter<Cursor<Vec<u8>>>>,
    stylesheet: Arc<Stylesheet>,
    layout_engine: LayoutEngine,
    font_map: HashMap<String, String>,
    page_content_ids: Vec<ObjectId>,
    toc_fixups: Vec<TocFixup>,
}

#[derive(Debug)]
struct TocFixup {
    global_page_idx: usize,
    rect: [f32; 4],
    style: Arc<ComputedStyle>,
}

impl<W: Write + Send> LopdfDocumentRenderer<W> {
    pub fn new(layout_engine: LayoutEngine, stylesheet: Stylesheet) -> Result<Self, RenderError> {
        let stylesheet = Arc::new(stylesheet);
        let mut font_map = HashMap::new();
        for (i, face) in layout_engine.font_manager.db().faces().enumerate() {
            font_map.insert(face.post_script_name.clone(), format!("F{}", i + 1));
        }
        Ok(Self {
            final_writer: None,
            buffer_writer: None,
            stylesheet,
            layout_engine,
            font_map,
            page_content_ids: Vec::new(),
            toc_fixups: Vec::new(),
        })
    }

    fn get_page_dimensions_pt(page_layout: &PageLayout) -> (f32, f32) {
        page_layout.size.dimensions_pt()
    }

    fn get_styled_font_name(style: &Arc<ComputedStyle>) -> String {
        let family = &style.font_family;
        match style.font_weight {
            FontWeight::Bold | FontWeight::Black => format!("{}-Bold", family),
            _ => family.to_string(),
        }
    }
}

impl<W: Write + Send> DocumentRenderer<W> for LopdfDocumentRenderer<W> {
    fn begin_document(&mut self, writer: W) -> Result<(), renderer::RenderError> {
        self.final_writer = Some(writer);
        let buffer = Cursor::new(Vec::new());
        let mut font_dict = Dictionary::new();
        for face in self.layout_engine.font_manager.db().faces() {
            if let Some(internal_name) = self.font_map.get(&face.post_script_name) {
                let single_font_dict = dictionary! {
                    "Type" => "Font", "Subtype" => "Type1", "BaseFont" => face.post_script_name.clone(), "Encoding" => "WinAnsiEncoding",
                };
                font_dict.set(internal_name.as_bytes(), Object::Dictionary(single_font_dict));
            }
        }
        self.buffer_writer = Some(StreamingPdfWriter::new(buffer, "1.7", font_dict)?);
        Ok(())
    }

    fn add_resources(&mut self, _resources: &HashMap<String, SharedData>) -> Result<(), renderer::RenderError> {
        Ok(())
    }

    fn render_page(&mut self, context: &Value, elements: Vec<PositionedElement>, template_engine: &Handlebars) -> Result<(), renderer::RenderError> {
        let writer = self.buffer_writer.as_mut().unwrap();
        let global_page_idx = self.page_content_ids.len();
        let default_master_name = self.stylesheet.default_page_master_name.as_ref().unwrap();
        let page_layout = self.stylesheet.page_masters.get(default_master_name).unwrap();
        let (page_width, page_height) = Self::get_page_dimensions_pt(page_layout);
        let mut page_ctx = PageContext::new(&self.layout_engine, page_width, page_height, &self.font_map, &self.stylesheet);
        for el in &elements {
            if let LayoutElement::TableOfContentsPlaceholder = &el.element {
                self.toc_fixups.push(TocFixup { global_page_idx, rect: [el.x, el.y, el.width, el.height], style: el.style.clone() });
                page_ctx.draw_background_and_borders(el)?;
            } else { page_ctx.draw_element(el)?; }
        }
        if let Some(footer_template) = &page_layout.footer_text {
            page_ctx.draw_footer(context, template_engine, footer_template, page_layout, global_page_idx + 1)?;
        }
        let content = page_ctx.finish();
        let content_id = writer.buffer_content_stream(content);
        self.page_content_ids.push(content_id);
        Ok(())
    }

    fn finalize(&mut self, resolved_anchors: &HashMap<String, ResolvedAnchor>, sequences: &[LaidOutSequence]) -> Result<(), RenderError> {
        let default_master_name = self.stylesheet.default_page_master_name.as_ref().unwrap();
        let page_layout = self.stylesheet.page_masters.get(default_master_name).unwrap();
        let (page_width, page_height) = Self::get_page_dimensions_pt(page_layout);

        // Pre-calculate all TOC layouts before getting a mutable borrow of the writer.
        let toc_layouts: Vec<_> = self.toc_fixups.iter().map(|fixup| {
            let elements = self.layout_toc_for_fixup(fixup, resolved_anchors, sequences);
            (fixup, elements)
        }).collect();

        if let Some(writer) = self.buffer_writer.as_mut() {
            let final_page_ids: Vec<ObjectId> = (0..self.page_content_ids.len()).map(|_| writer.new_object_id()).collect();
            let mut toc_content_streams = HashMap::new();
            let mut link_annots_by_page: HashMap<usize, Vec<ObjectId>> = HashMap::new();

            for (fixup, toc_elements) in &toc_layouts {
                if toc_elements.is_empty() { continue; }

                let mut toc_content = Content { operations: vec![] };
                let mut toc_page_ctx = PageContext::new(&self.layout_engine, page_width, page_height, &self.font_map, &self.stylesheet);
                let page_annots = link_annots_by_page.entry(fixup.global_page_idx).or_default();
                for el in toc_elements {
                    if let LayoutElement::Text(t) = &el.element {
                        if let Some(href) = &t.href {
                            if let Some(target_id) = href.strip_prefix('#') {
                                if let Some(anchor) = resolved_anchors.get(target_id) {
                                    if anchor.global_page_index > 0 && anchor.global_page_index <= final_page_ids.len() {
                                        let target_page_id = final_page_ids[anchor.global_page_index - 1];
                                        let y_dest = page_height - anchor.y_pos;
                                        log::debug!(
                                            "Creating TOC link for anchor '{}': page_idx={}, y_pos={}, target_page_id={:?}, y_dest={}",
                                            target_id, anchor.global_page_index, anchor.y_pos, target_page_id, y_dest
                                        );
                                        let dest = vec![Object::Reference(target_page_id), "FitH".into(), y_dest.into()];
                                        let action = dictionary! { "Type" => "Action", "S" => "GoTo", "D" => dest };
                                        let action_id = writer.buffer_object(action.into());
                                        let rect = vec![el.x.into(), (page_height - (el.y + el.height)).into(), (el.x + el.width).into(), (page_height - el.y).into()];
                                        let annot = dictionary! { "Type" => "Annot", "Subtype" => "Link", "Rect" => rect, "Border" => vec![0.into(), 0.into(), 0.into()], "A" => action_id };
                                        let annot_id = writer.buffer_object(annot.into());
                                        page_annots.push(annot_id);
                                    }
                                }
                            }
                        }
                    }
                    toc_page_ctx.draw_element(el)?;
                }
                toc_content.operations.extend(toc_page_ctx.finish().operations);
                toc_content_streams.insert(fixup.global_page_idx, writer.buffer_content_stream(toc_content));
            }

            let outline_root_id = Self::build_outlines(sequences, resolved_anchors, &final_page_ids, page_height, writer);

            for (i, page_id) in final_page_ids.iter().enumerate() {
                let mut contents_array = vec![Object::Reference(self.page_content_ids[i])];
                if let Some(toc_stream_id) = toc_content_streams.get(&i) {
                    contents_array.push(Object::Reference(*toc_stream_id));
                }
                let mut page_dict = dictionary! {
                    "Type" => "Page", "Parent" => writer.pages_id,
                    "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
                    "Contents" => Object::Array(contents_array), "Resources" => writer.resources_id,
                };
                if let Some(annots) = link_annots_by_page.get(&i) {
                    page_dict.set("Annots", annots.iter().map(|id| Object::Reference(*id)).collect::<Vec<Object>>());
                }
                writer.buffer_object_at_id(*page_id, page_dict.into());
            }

            writer.set_page_ids(final_page_ids);
            writer.set_outline_root_id(outline_root_id);
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<(), renderer::RenderError> {
        if let Some(writer) = self.buffer_writer.take() {
            let buffer = writer.finish()?;
            if let Some(final_writer) = self.final_writer.as_mut() {
                final_writer.write_all(buffer.get_ref())?;
            }
        }
        Ok(())
    }
}

impl<W: Write + Send> LopdfDocumentRenderer<W> {
    fn layout_toc_for_fixup(&self, fixup: &TocFixup, resolved_anchors: &HashMap<String, ResolvedAnchor>, sequences: &[LaidOutSequence]) -> Vec<PositionedElement> {
        let mut toc_items_ir = Vec::new();
        for seq in sequences {
            for entry in &seq.toc_entries {
                if let Some(anchor) = resolved_anchors.get(&entry.target_id) {
                    toc_items_ir.push(create_toc_entry_ir(entry, anchor.global_page_index, &fixup.style));
                }
            }
        }
        if toc_items_ir.is_empty() { return vec![]; }
        let toc_root = IRNode::Block { meta: NodeMetadata::default(), children: toc_items_ir };
        let mut toc_layout_node = self.layout_engine.build_layout_node_tree(&toc_root, fixup.style.clone());
        toc_layout_node.measure(&self.layout_engine, fixup.rect[2]);
        let toc_elements_cell = std::cell::RefCell::new(Vec::new());
        let dummy_anchors = std::cell::RefCell::new(HashMap::new());
        let bounds = crate::core::layout::geom::Rect { x: 0.0, y: 0.0, width: fixup.rect[2], height: fixup.rect[3] };
        let mut layout_ctx = crate::core::layout::node::LayoutContext::new(&self.layout_engine, bounds, &toc_elements_cell, &dummy_anchors);
        if let Err(e) = toc_layout_node.layout(&mut layout_ctx) { log::error!("Failed to layout Table of Contents: {}", e); }
        toc_elements_cell.into_inner().into_iter().map(|mut el| {
            el.x += fixup.rect[0];
            el.y += fixup.rect[1];
            el
        }).collect()
    }

    fn build_outlines(sequences: &[LaidOutSequence], resolved_anchors: &HashMap<String, ResolvedAnchor>, final_page_ids: &[ObjectId], page_height: f32, writer: &mut StreamingPdfWriter<Cursor<Vec<u8>>>) -> Option<ObjectId> {
        if sequences.is_empty() || sequences.iter().all(|s| s.toc_entries.is_empty()) {
            return None;
        }

        // The final tree node structure.
        struct NodeOutlineItem {
            id: ObjectId,
            title: String,
            dest: Vec<Object>,
            children: Vec<NodeOutlineItem>,
        }

        // A temporary struct for the flat list.
        struct FlatOutlineItem {
            id: ObjectId,
            title: String,
            dest: Vec<Object>,
            parent_idx: Option<usize>,
        }

        let mut flat_list = Vec::new();
        // Stack stores (level, index_in_flat_list)
        let mut level_stack: Vec<(u8, usize)> = vec![(0, usize::MAX)]; // Use usize::MAX as a sentinel for root.

        for entry in sequences.iter().flat_map(|s| &s.toc_entries) {
            if let Some(anchor) = resolved_anchors.get(&entry.target_id) {
                if anchor.global_page_index == 0 || anchor.global_page_index > final_page_ids.len() { continue; }
                let dest_page_id = final_page_ids[anchor.global_page_index - 1];
                let y_dest = page_height - anchor.y_pos;
                log::debug!(
                    "Creating outline for anchor '{}': page_idx={}, y_pos={}, target_page_id={:?}, y_dest={}",
                    entry.target_id, anchor.global_page_index, anchor.y_pos, dest_page_id, y_dest
                );
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

        if flat_list.is_empty() { return None; }

        // Reconstruct the tree from the flat list.
        let mut children_map: HashMap<usize, Vec<NodeOutlineItem>> = HashMap::new();
        let mut root_items: Vec<NodeOutlineItem> = Vec::new();

        for (i, flat_node) in flat_list.into_iter().enumerate().rev() {
            let mut children = children_map.remove(&i).unwrap_or_default();
            children.reverse();

            let node = NodeOutlineItem {
                id: flat_node.id,
                title: flat_node.title,
                dest: flat_node.dest,
                children,
            };

            if let Some(parent_idx) = flat_node.parent_idx {
                children_map.entry(parent_idx).or_default().push(node);
            } else {
                root_items.push(node);
            }
        }
        root_items.reverse();

        // Now, buffer the objects for the PDF.
        let outline_root_id = writer.new_object_id();
        let first_id = root_items.first().unwrap().id;
        let last_id = root_items.last().unwrap().id;
        writer.buffer_object_at_id(outline_root_id, dictionary!{
            "Type" => "Outlines", "First" => first_id, "Last" => last_id, "Count" => root_items.len() as i64,
        }.into());

        // Recursive function to write the tree nodes.
        fn buffer_outline_level<W: Write+Send+Seek>(items: &[NodeOutlineItem], parent_id: ObjectId, writer: &mut StreamingPdfWriter<W>) {
            for (i, item) in items.iter().enumerate() {
                let mut dict = dictionary!{
                    "Title" => Object::String(to_win_ansi(&item.title), StringFormat::Literal),
                    "Parent" => parent_id, "Dest" => item.dest.clone(),
                };
                if i > 0 { dict.set("Prev", items[i-1].id); }
                if i < items.len() - 1 { dict.set("Next", items[i+1].id); }
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
        Some(outline_root_id)
    }
}

fn create_toc_entry_ir(entry: &TocEntry, page_num: usize, base_style: &Arc<ComputedStyle>) -> IRNode {
    let indent = (entry.level.saturating_sub(1) as f32) * 15.0;
    let container_style = ElementStyle { margin: Some(Margins { left: indent, bottom: 4.0, ..Default::default() }), flex_direction: Some(crate::core::style::flex::FlexDirection::Row), align_items: Some(crate::core::style::flex::AlignItems::Baseline), ..Default::default() };
    IRNode::FlexContainer {
        meta: NodeMetadata { style_override: Some(container_style), ..Default::default() },
        children: vec![
            IRNode::Paragraph {
                meta: NodeMetadata { style_override: Some(ElementStyle{ flex_shrink: Some(0.0), ..Default::default() }), ..Default::default()},
                children: vec![InlineNode::Hyperlink { href: format!("#{}", entry.target_id), meta: Default::default(), children: vec![InlineNode::Text(entry.text.clone())]}],
            },
            IRNode::Block {
                meta: NodeMetadata { style_override: Some(ElementStyle { flex_grow: Some(1.0), margin: Some(Margins::x(5.0)), height: Some(Dimension::Pt(base_style.font_size * 0.7)), border_bottom: Some(crate::core::style::border::Border { width: 1.0, style: crate::core::style::border::BorderStyle::Dotted, color: Color::gray(150), }), ..Default::default() }), ..Default::default() },
                children: vec![],
            },
            IRNode::Paragraph {
                meta: Default::default(),
                children: vec![InlineNode::Hyperlink { href: format!("#{}", entry.target_id), meta: Default::default(), children: vec![InlineNode::Text(page_num.to_string())]}],
            },
        ],
    }
}

fn to_win_ansi(s: &str) -> Vec<u8> {
    s.chars().map(|c| if c as u32 <= 255 { c as u8 } else { b'?' }).collect()
}

struct PageContext<'a> { layout_engine: &'a LayoutEngine, page_width: f32, page_height: f32, content: Content, state: LopdfPageRenderState, font_map: &'a HashMap<String, String>, stylesheet: &'a Stylesheet }
#[derive(Default, Clone, PartialEq)]
struct LopdfPageRenderState { font_name: String, font_size: f32, fill_color: Color }

impl<'a> PageContext<'a> {
    fn new(layout_engine: &'a LayoutEngine, page_width: f32, page_height: f32, font_map: &'a HashMap<String, String>, stylesheet: &'a Stylesheet) -> Self {
        Self { layout_engine, page_width, page_height, content: Content { operations: vec![] }, state: Default::default(), font_map, stylesheet }
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
        let style = &el.style; let x = el.x; let y = self.page_height - (el.y + el.height);
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
    fn set_font(&mut self, style: &Arc<ComputedStyle>) {
        let styled_font_name = LopdfDocumentRenderer::<Cursor<Vec<u8>>>::get_styled_font_name(style);
        let internal_font_name = self.font_map.get(&styled_font_name).or_else(|| self.font_map.get(style.font_family.as_str())).unwrap_or(&DEFAULT_LOPDF_FONT_NAME);
        if self.state.font_name != *internal_font_name || self.state.font_size != style.font_size {
            self.content.operations.push(Operation::new("Tf", vec![Object::Name(internal_font_name.as_bytes().to_vec()), style.font_size.into()]));
            self.state.font_name = internal_font_name.to_string(); self.state.font_size = style.font_size;
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
        self.set_font(&el.style); self.set_fill_color(&el.style.color);
        let baseline_y = el.y + el.style.font_size * 0.8; let pdf_y = self.page_height - baseline_y;
        self.content.operations.push(Operation::new("Td", vec![el.x.into(), pdf_y.into()]));
        self.content.operations.push(Operation::new("Tj", vec![Object::String(to_win_ansi(&text.content), StringFormat::Literal)]));
        self.content.operations.push(Operation::new("ET", vec![]));
        Ok(())
    }
    fn draw_image(&mut self, image: &ImageElement, _el: &PositionedElement) -> Result<(), RenderError> { log::warn!("Images are not supported in the lopdf streaming renderer yet: {}", image.src); Ok(()) }
    fn draw_footer(&mut self, context: &Value, template_engine: &Handlebars, footer_template: &str, page_layout: &PageLayout, page_num: usize) -> Result<(), RenderError> {
        let margins = page_layout.margins.as_ref().cloned().unwrap_or_default();
        let style_sets = if let Some(style_name) = page_layout.footer_style.as_deref() { self.stylesheet.styles.get(style_name).map(|s| vec![s.clone()]).unwrap_or_default() } else { vec![] };
        let style = self.layout_engine.compute_style(&style_sets, None, &self.layout_engine.get_default_style());
        #[derive(serde::Serialize)] struct Ctx<'a> { page_num: usize, #[serde(flatten)] data: &'a Value }
        let text = template_engine.render_template(footer_template, &Ctx { page_num, data: context })?;
        let page_width = self.page_width;
        self.content.operations.push(Operation::new("BT", vec![]));
        self.set_font(&style); self.set_fill_color(&style.color);
        let line_width = self.layout_engine.measure_text_width(&text, &style);
        let y = margins.bottom - style.font_size;
        let x = match style.text_align {
            crate::core::style::text::TextAlign::Left => margins.left,
            crate::core::style::text::TextAlign::Right => page_width - margins.right - line_width,
            crate::core::style::text::TextAlign::Center => margins.left + (page_width - margins.left - margins.right - line_width) / 2.0,
            crate::core::style::text::TextAlign::Justify => margins.left,
        };
        self.content.operations.push(Operation::new("Td", vec![x.into(), y.into()]));
        self.content.operations.push(Operation::new("Tj", vec![Object::String(to_win_ansi(&text), StringFormat::Literal)]));
        self.content.operations.push(Operation::new("ET", vec![]));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::fonts::FontManager;
    use crate::core::style::dimension::PageSize;

    fn create_test_engine() -> LayoutEngine {
        let mut font_manager = FontManager::new();
        font_manager.load_fallback_font();
        LayoutEngine::new(Arc::new(font_manager))
    }

    #[test]
    fn test_toc_and_outline_links_are_generated_correctly() -> Result<(), RenderError> {
        let engine = create_test_engine();
        let stylesheet = Stylesheet {
            page_masters: HashMap::from([(
                "default".to_string(),
                PageLayout { size: PageSize::A4, ..Default::default() },
            )]),
            default_page_master_name: Some("default".to_string()),
            ..Default::default()
        };
        let (_page_width, page_height) = PageSize::A4.dimensions_pt();

        // 1. Setup mock data from layout stage
        let mut resolved_anchors = HashMap::new();
        resolved_anchors.insert(
            "section1".to_string(),
            ResolvedAnchor { global_page_index: 2, y_pos: 100.0 },
        );

        let sequences = vec![LaidOutSequence {
            pages: vec![], // Not used by finalize for this test
            toc_entries: vec![TocEntry {
                level: 1,
                text: "Go to Section 1".to_string(),
                target_id: "section1".to_string(),
            }],
            ..Default::default()
        }];
        let default_style = engine.get_default_style();

        // 2. Setup and run the renderer
        let mut renderer = LopdfDocumentRenderer::new(engine, stylesheet)?;
        let mut writer = Cursor::new(Vec::new());
        renderer.begin_document(&mut writer)?;

        // Simulate rendering two pages. Page 1 has the TOC.
        let toc_placeholder = PositionedElement {
            x: 50.0, y: 50.0, width: 300.0, height: 200.0,
            element: LayoutElement::TableOfContentsPlaceholder,
            style: default_style.clone(),
        };
        renderer.render_page(&Value::Null, vec![toc_placeholder], &Handlebars::new())?;
        renderer.render_page(&Value::Null, vec![], &Handlebars::new())?;

        renderer.finalize(&resolved_anchors, &sequences)?;
        let doc_box = Box::new(renderer);
        doc_box.finish()?;

        // 3. Parse the generated PDF and verify links
        let pdf_bytes = writer.into_inner();
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("Failed to parse generated PDF");
        let pages = doc.get_pages();
        let page2_id = pages.get(&2).expect("Page 2 should exist");

        // Verify Outline Link
        let catalog = doc.catalog().unwrap();
        let outlines_ref = catalog.get(b"Outlines").unwrap().as_reference().unwrap();
        let outlines = doc.get_object(outlines_ref).unwrap().as_dict().unwrap();
        let first_item_ref = outlines.get(b"First").unwrap().as_reference().unwrap();
        let first_item = doc.get_object(first_item_ref).unwrap().as_dict().unwrap();
        let dest = first_item.get(b"Dest").unwrap().as_array().unwrap();

        assert_eq!(dest[0].as_reference().unwrap(), *page2_id, "Outline should point to page 2");
        assert_eq!(dest[1].as_name().unwrap(), b"FitH", "Outline destination should use /FitH");
        assert!((dest[2].as_f32().unwrap() - (page_height - 100.0)).abs() < 0.1, "Outline y-pos is incorrect");

        // Verify TOC Link Annotation on Page 1
        let page1_id = pages.get(&1).unwrap();
        let page1_obj = doc.get_object(*page1_id).unwrap().as_dict().unwrap();
        let annots_array = page1_obj.get(b"Annots").unwrap().as_array().unwrap();
        assert!(!annots_array.is_empty(), "Page 1 should have link annotations");
        let link_annot_ref = annots_array[0].as_reference().unwrap();
        let link_annot = doc.get_object(link_annot_ref).unwrap().as_dict().unwrap();
        let action_ref = link_annot.get(b"A").unwrap().as_reference().unwrap();
        let action = doc.get_object(action_ref).unwrap().as_dict().unwrap();
        let dest_from_action = action.get(b"D").unwrap().as_array().unwrap();

        assert_eq!(dest_from_action[0].as_reference().unwrap(), *page2_id, "TOC link should point to page 2");
        assert_eq!(dest_from_action[1].as_name().unwrap(), b"FitH", "TOC link destination should use /FitH");
        assert!((dest_from_action[2].as_f32().unwrap() - (page_height - 100.0)).abs() < 0.1, "TOC link y-pos is incorrect");

        Ok(())
    }
}