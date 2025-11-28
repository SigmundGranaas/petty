// src/core/layout/text.rs

use crate::core::idf::InlineNode;
// Fix: Use the public re-export from layout/mod.rs to avoid unresolved import errors
// caused by the engine module being private.
use crate::core::layout::LayoutEngine;
use crate::core::layout::nodes::image::ImageNode;
use crate::core::layout::style::ComputedStyle;
use bumpalo::Bump;
use cosmic_text::AttrsList;
use std::sync::Arc;

pub struct TextBuilder<'a, 'b> {
    engine: &'a LayoutEngine,
    arena: &'b Bump, // Needed to build inline images in the arena
    pub content: String,
    pub attrs_list: AttrsList,
    pub links: Vec<String>,
    // Inline images need to be allocated in the arena
    pub inline_images: Vec<(usize, &'b ImageNode)>,
}

impl<'a, 'b> TextBuilder<'a, 'b> {
    pub fn new(engine: &'a LayoutEngine, arena: &'b Bump, base_style: &Arc<ComputedStyle>) -> Self {
        let attrs = engine.font_manager.attrs_from_style(base_style);
        Self {
            engine,
            arena,
            content: String::new(),
            attrs_list: AttrsList::new(&attrs),
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
        current_link_idx: usize,
    ) {
        for node in inlines {
            match node {
                InlineNode::Text(text) => {
                    let start = self.content.len();
                    self.content.push_str(text);
                    let end = self.content.len();

                    let attrs = self.engine.font_manager.attrs_from_style(parent_style);
                    let mut attrs_clone = attrs;
                    attrs_clone.metadata = current_link_idx;
                    self.attrs_list.add_span(start..end, &attrs_clone);
                }
                InlineNode::StyledSpan { meta, children } => {
                    let style = self.resolve_meta_style(meta, parent_style);
                    self.process_inlines_recursive(children, &style, current_link_idx);
                }
                InlineNode::Hyperlink {
                    meta,
                    children,
                    href,
                } => {
                    let style = self.resolve_meta_style(meta, parent_style);
                    self.links.push(href.to_string());
                    let new_link_idx = self.links.len(); // 1-based index
                    self.process_inlines_recursive(children, &style, new_link_idx);
                }
                InlineNode::PageReference { meta, children, .. } => {
                    let style = self.resolve_meta_style(meta, parent_style);
                    self.process_inlines_recursive(children, &style, current_link_idx);
                }
                InlineNode::LineBreak => {
                    let start = self.content.len();
                    self.content.push('\n');
                    let end = self.content.len();
                    let attrs = self.engine.font_manager.attrs_from_style(parent_style);
                    let mut attrs_clone = attrs;
                    attrs_clone.metadata = current_link_idx;
                    self.attrs_list.add_span(start..end, &attrs_clone);
                }
                InlineNode::Image { meta, src } => {
                    let start = self.content.len();
                    self.content.push_str("\u{FFFC}");
                    let end = self.content.len();

                    // Create inline image allocated in Arena
                    if let Ok(node) = ImageNode::new_inline(meta, src.clone(), self.engine, parent_style, self.arena) {
                        // We store reference to the arena-allocated node
                        let node_ref = self.arena.alloc(node);
                        self.inline_images.push((start, node_ref));

                        let attrs = self.engine.font_manager.attrs_from_style(parent_style);
                        let mut attrs_clone = attrs;
                        attrs_clone.metadata = 1 << 31 | self.inline_images.len();
                        self.attrs_list.add_span(start..end, &attrs_clone);
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
        self.engine
            .compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style)
    }
}