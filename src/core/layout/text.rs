use crate::core::idf::InlineNode;
use crate::core::layout::engine::LayoutEngine;
use crate::core::layout::style::ComputedStyle;
use cosmic_text::{AttrsList};
use std::sync::Arc;

/// A helper to flatten a tree of `InlineNode`s into a plain string and a list of attributes
/// (spans) suitable for `cosmic_text`.
pub struct TextBuilder<'a> {
    engine: &'a LayoutEngine,
    pub content: String,
    pub attrs_list: AttrsList,
    /// Stores link targets. The index in this vector corresponds to `Attrs::metadata`.
    /// 0 is reserved for "no link". metadata 1 maps to index 0 in this vector.
    pub links: Vec<String>,
}

impl<'a> TextBuilder<'a> {
    pub fn new(engine: &'a LayoutEngine, base_style: &Arc<ComputedStyle>) -> Self {
        let base_attrs = engine.font_manager.attrs_from_style(base_style);
        Self {
            engine,
            content: String::new(),
            attrs_list: AttrsList::new(&base_attrs),
            links: Vec::new(),
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

                    // Register new link
                    self.links.push(href.clone());
                    let new_link_idx = self.links.len(); // 1-based index for metadata

                    self.process_inlines_recursive(children, &style, new_link_idx);
                }
                InlineNode::PageReference { meta, children, .. } => {
                    let style = self.resolve_meta_style(meta, parent_style);
                    if children.is_empty() {
                        // Insert placeholder text for page numbers
                        let start = self.content.len();
                        self.content.push_str("XX");
                        let end = self.content.len();
                        let mut attrs = self.engine.font_manager.attrs_from_style(&style);
                        attrs.metadata = current_link_idx;
                        self.attrs_list.add_span(start..end, &attrs);
                    } else {
                        self.process_inlines_recursive(children, &style, current_link_idx);
                    }
                }
                InlineNode::LineBreak => {
                    let start = self.content.len();
                    self.content.push('\n');
                    let end = self.content.len();

                    // Ensure the newline character is covered by a span so cosmic-text
                    // processes it correctly as part of the text flow.
                    let mut attrs = self.engine.font_manager.attrs_from_style(parent_style);
                    attrs.metadata = current_link_idx;
                    self.attrs_list.add_span(start..end, &attrs);
                }
                InlineNode::Image { .. } => {
                    // Images in text flow not supported yet
                    log::warn!("Inline images are not supported in the current text engine refactor.");
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