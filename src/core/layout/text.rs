// src/core/layout/text.rs

use crate::core::idf::InlineNode;
use crate::core::layout::LayoutEngine;
use crate::core::layout::nodes::image::ImageNode;
use crate::core::layout::style::ComputedStyle;
use crate::core::layout::engine::LayoutStore;
use std::sync::Arc;

/// Represents a contiguous run of text with a single style.
#[derive(Debug, Clone)]
pub struct TextSpan<'a> {
    pub text: &'a str,
    pub style: Arc<ComputedStyle>,
    pub link_index: usize, // 0 if no link, 1-based index into builder.links
}

/// Represents an inline image embedded in the text.
#[derive(Debug, Clone)]
pub struct InlineImageEntry<'a> {
    pub index: usize, // Character index in the full concatenated string
    pub node: &'a ImageNode<'a>,
}

pub struct TextBuilder<'a, 'b> {
    engine: &'a LayoutEngine,
    store: &'b LayoutStore,
    raw_content: String,
    // Store Arc<ComputedStyle> to facilitate cheap cloning for cache keys
    span_ranges: Vec<(std::ops::Range<usize>, Arc<ComputedStyle>, usize)>,

    pub links: Vec<String>,
    pub inline_images: Vec<InlineImageEntry<'b>>,
}

impl<'a, 'b> TextBuilder<'a, 'b> {
    pub fn new(engine: &'a LayoutEngine, store: &'b LayoutStore, _base_style: &Arc<ComputedStyle>) -> Self {
        Self {
            engine,
            store,
            raw_content: String::new(),
            span_ranges: Vec::new(),
            links: Vec::new(),
            inline_images: Vec::new(),
        }
    }

    pub fn process_inlines(&mut self, inlines: &[InlineNode], parent_style: &Arc<ComputedStyle>) {
        self.process_inlines_recursive(inlines, parent_style, 0);
    }

    /// Consumes the builder and returns the full text, list of spans, inline images, and links.
    pub fn finish(self) -> (&'b str, &'b [TextSpan<'b>], Vec<InlineImageEntry<'b>>, Vec<String>) {
        let full_text = self.store.alloc_str(&self.raw_content);

        let mut spans = Vec::with_capacity(self.span_ranges.len());
        for (range, style, link_idx) in self.span_ranges {
            spans.push(TextSpan {
                text: &full_text[range],
                style, // Cheap move of the Arc
                link_index: link_idx,
            });
        }

        (
            full_text,
            self.store.bump.alloc_slice_clone(&spans),
            self.inline_images,
            self.links
        )
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
                    let start = self.raw_content.len();
                    self.raw_content.push_str(text);
                    let end = self.raw_content.len();

                    let style = parent_style.clone();

                    // OPTIMIZATION: Attempt to merge with the previous span
                    let mut merged = false;
                    if let Some(last) = self.span_ranges.last_mut() {
                        // Check 1: Styles are effectively the same object (pointer equality is fast)
                        // Check 2: Link index matches
                        if Arc::ptr_eq(&last.1, &style) && last.2 == current_link_idx {
                            // Check 3: Ensure we don't merge into an Image span
                            let is_image_span = if let Some(last_img) = self.inline_images.last() {
                                last_img.index == last.0.start
                            } else {
                                false
                            };

                            if !is_image_span {
                                last.0.end = end;
                                merged = true;
                            }
                        }
                    }

                    if !merged {
                        self.span_ranges.push((start..end, style, current_link_idx));
                    }
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
                    let start = self.raw_content.len();
                    self.raw_content.push('\n');
                    let end = self.raw_content.len();
                    let style = parent_style.clone();
                    self.span_ranges.push((start..end, style, current_link_idx));
                }
                InlineNode::Image { meta, src } => {
                    let start = self.raw_content.len();
                    self.raw_content.push_str("\u{FFFC}");
                    let end = self.raw_content.len();

                    if let Ok(node) = ImageNode::new_inline(meta, src.clone(), self.engine, parent_style, self.store) {
                        let node_ref = self.store.bump.alloc(node);
                        self.inline_images.push(InlineImageEntry {
                            index: start,
                            node: node_ref,
                        });
                        let style = parent_style.clone();
                        self.span_ranges.push((start..end, style, current_link_idx));
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