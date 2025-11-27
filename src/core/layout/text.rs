use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::nodes::image::ImageNode;
use cosmic_text::{AttrsList};
use std::sync::Arc;
use crate::core::idf::InlineNode;

pub struct TextBuilder<'a> {
    engine: &'a LayoutEngine,
    pub content: String,
    pub attrs_list: AttrsList,
    pub links: Vec<String>,
    pub inline_images: Vec<(usize, ImageNode)>,
}

impl<'a> TextBuilder<'a> {
    pub fn new(engine: &'a LayoutEngine, base_style: &Arc<ComputedStyle>) -> Self {
        let base_attrs = engine.font_manager.attrs_from_style(base_style);
        Self {
            engine,
            content: String::new(),
            attrs_list: AttrsList::new(&base_attrs),
            links: Vec::new(),
            inline_images: Vec::new(),
        }
    }

    pub fn process_inlines(&mut self, inlines: &[InlineNode], parent_style: &Arc<ComputedStyle>) {
        self.process_inlines_recursive(inlines, parent_style, 0);
    }

    fn process_inlines_recursive(
        &mut self,
        inlines: &[InlineNode],
        parent_style: &Arc<ComputedStyle>,
        current_link_idx: usize
    ) {
        for node in inlines {
            match node {
                InlineNode::Text(text) => {
                    let start = self.content.len();
                    self.content.push_str(text);
                    let end = self.content.len();

                    let mut attrs = self.engine.font_manager.attrs_from_style(parent_style);
                    attrs.metadata = current_link_idx;
                    self.attrs_list.add_span(start..end, &attrs);
                }
                InlineNode::StyledSpan { meta, children } => {
                    let style = self.resolve_meta_style(meta, parent_style);
                    self.process_inlines_recursive(children, &style, current_link_idx);
                }
                InlineNode::Hyperlink { meta, children, href } => {
                    let style = self.resolve_meta_style(meta, parent_style);
                    self.links.push(href.clone());
                    let new_link_idx = self.links.len(); // 1-based index
                    self.process_inlines_recursive(children, &style, new_link_idx);
                }
                InlineNode::PageReference { meta, children, .. } => {
                    // .. same as text ..
                    let style = self.resolve_meta_style(meta, parent_style);
                    self.process_inlines_recursive(children, &style, current_link_idx);
                }
                InlineNode::LineBreak => {
                    let start = self.content.len();
                    self.content.push('\n');
                    let end = self.content.len();
                    let mut attrs = self.engine.font_manager.attrs_from_style(parent_style);
                    attrs.metadata = current_link_idx;
                    self.attrs_list.add_span(start..end, &attrs);
                }
                InlineNode::Image { meta, src } => {
                    // Inline Image Support (Issue F)
                    // Insert placeholder char
                    let start = self.content.len();
                    self.content.push_str("\u{FFFC}");
                    let end = self.content.len();

                    let node = ImageNode::new_inline(meta, src.clone(), self.engine, parent_style).ok();

                    if let Some(n) = node {
                        // Store mapping
                        self.inline_images.push((start, n));

                        // Set metadata to indicate image (High bit)
                        let mut attrs = self.engine.font_manager.attrs_from_style(parent_style);
                        // Using a simple flag in metadata.
                        // Real implementation would index into inline_images.
                        attrs.metadata = 1 << 31 | self.inline_images.len();
                        self.attrs_list.add_span(start..end, &attrs);
                    }
                }
            }
        }
    }

    fn resolve_meta_style(
        &self,
        meta: &crate::core::idf::InlineMetadata,
        parent_style: &Arc<ComputedStyle>,
    ) -> Arc<ComputedStyle> {
        self.engine.compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style)
    }
}