use crate::LayoutEngine;
use crate::engine::LayoutStore;
use crate::nodes::image::ImageNode;
use crate::style::ComputedStyle;
use petty_idf::InlineNode;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TextSpan<'a> {
    pub text: &'a str,
    pub style: Arc<ComputedStyle>,
    pub link_index: usize,
}

#[derive(Debug, Clone)]
pub struct InlineImageEntry<'a> {
    pub index: usize,
    pub node: &'a ImageNode<'a>,
}

pub struct TextBuilder<'a, 'b> {
    engine: &'a LayoutEngine,
    store: &'b LayoutStore,
    raw_content: String,
    span_ranges: Vec<(std::ops::Range<usize>, Arc<ComputedStyle>, usize)>,
    pub links: Vec<String>,
    pub inline_images: Vec<InlineImageEntry<'b>>,
}

impl<'a, 'b> TextBuilder<'a, 'b> {
    pub fn new(
        engine: &'a LayoutEngine,
        store: &'b LayoutStore,
        _base_style: &Arc<ComputedStyle>,
    ) -> Self {
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

    pub fn finish(
        self,
    ) -> (
        &'b str,
        &'b [TextSpan<'b>],
        Vec<InlineImageEntry<'b>>,
        Vec<String>,
    ) {
        let full_text = self.store.alloc_str(&self.raw_content);
        let mut spans = Vec::with_capacity(self.span_ranges.len());
        for (range, style, link_idx) in self.span_ranges {
            spans.push(TextSpan {
                text: &full_text[range],
                style,
                link_index: link_idx,
            });
        }
        (
            full_text,
            self.store.bump.alloc_slice_clone(&spans),
            self.inline_images,
            self.links,
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

                    let mut merged = false;
                    #[allow(clippy::collapsible_if)]
                    if let Some(last) = self.span_ranges.last_mut() {
                        if Arc::ptr_eq(&last.1, &style) && last.2 == current_link_idx {
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
                    let new_link_idx = self.links.len();
                    self.process_inlines_recursive(children, &style, new_link_idx);
                }
                InlineNode::PageReference {
                    meta,
                    children,
                    target_id,
                } => {
                    let style = self.resolve_meta_style(meta, parent_style);
                    // PageReference is an internal link - add '#' prefix to target_id
                    self.links.push(format!("#{}", target_id));
                    let new_link_idx = self.links.len();
                    self.process_inlines_recursive(children, &style, new_link_idx);
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
                    self.raw_content.push('\u{FFFC}');
                    let end = self.raw_content.len();
                    if let Ok(node) = ImageNode::new_inline(
                        meta,
                        src.clone(),
                        self.engine,
                        parent_style,
                        self.store,
                    ) {
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
        meta: &petty_idf::InlineMetadata,
        parent_style: &Arc<ComputedStyle>,
    ) -> Arc<ComputedStyle> {
        self.engine
            .compute_style(&meta.style_sets, meta.style_override.as_ref(), parent_style)
    }
}
