use super::node::ParagraphNode;
use crate::LayoutError;
use crate::engine::{LayoutEngine, LayoutStore};
use crate::nodes::RenderNode;
use crate::style::ComputedStyle;
use crate::text::builder::TextBuilder;
use petty_idf::IRNode;
use std::sync::Arc;

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
        let id_ref = meta.id.as_ref().map(|s| store.alloc_str(s));

        let node = store.bump.alloc(Self {
            unique_id,
            id: id_ref,
            spans,
            full_text,
            links: links_slice,
            inline_images: images_slice,
            style: style_ref,
        });

        Ok(RenderNode::Paragraph(node))
    }
}
