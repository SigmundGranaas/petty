// src/pipeline/worker.rs
use crate::core::idf::{IRNode, InlineNode, SharedData};
use crate::core::layout::{AnchorLocation, LayoutEngine, PositionedElement};
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use log::{debug, info};
use rand::Rng;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Represents the output of a single worker task: the original data context,
/// the resulting pages of positioned elements, and all loaded resources.
#[derive(Debug, Clone, Default)]
pub(crate) struct LaidOutSequence {
    pub context: Arc<Value>,
    pub pages: Vec<Vec<PositionedElement>>,
    pub resources: HashMap<String, SharedData>,
    pub defined_anchors: HashMap<String, AnchorLocation>,
    pub toc_entries: Vec<TocEntry>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TocEntry {
    pub level: u8,
    pub text: String,
    pub target_id: String,
}

/// The second half of a worker's job: takes a parsed IR tree and performs
/// resource loading and layout. This part is generic over the template language.
pub(super) fn finish_layout_and_resource_loading(
    worker_id: usize,
    ir_nodes: Vec<IRNode>,
    context_arc: Arc<Value>,
    resource_base_path: &Path,
    layout_engine: &LayoutEngine,
    stylesheet: &Stylesheet,
    debug_mode: bool,
) -> Result<LaidOutSequence, PipelineError> {
    let total_start = Instant::now();
    let mut ir_nodes_with_ids = ir_nodes;
    ensure_heading_ids(&mut ir_nodes_with_ids);
    let tree = IRNode::Root(ir_nodes_with_ids.clone()); // TODO: Avoid clone

    if debug_mode {
        debug!("[WORKER-{}] Intermediate Representation (IR) tree dump:\n{:#?}", worker_id, &tree);
    }

    let resource_start = Instant::now();
    debug!("[WORKER-{}] Collecting and loading resources relative to '{}'.", worker_id, resource_base_path.display());
    let resources = collect_and_load_resources(&tree, resource_base_path)?;
    debug!("[WORKER-{}] Finished loading {} resources in {:.2?}.", worker_id, resources.len(), resource_start.elapsed());

    let mut toc_entries = Vec::new();
    collect_toc_entries(&tree, &mut toc_entries);

    let layout_start = Instant::now();
    debug!("[WORKER-{}] Paginating sequence tree.", worker_id);
    let (pages, defined_anchors) = layout_engine.paginate(stylesheet, ir_nodes_with_ids)?;
    debug!("[WORKER-{}] Finished paginating sequence ({} pages) in {:.2?}.", worker_id, pages.len(), layout_start.elapsed());

    info!("[WORKER-{}] Finished processing sequence in {:.2?}.", worker_id, total_start.elapsed());

    Ok(LaidOutSequence {
        context: context_arc,
        pages,
        resources,
        defined_anchors,
        toc_entries,
    })
}

/// Recursively ensures that all headings have a unique ID for anchor generation.
fn ensure_heading_ids(nodes: &mut [IRNode]) {
    for node in nodes {
        match node {
            IRNode::Heading { meta, children, .. } => {
                if meta.id.is_none() {
                    let text = extract_text_from_inlines(children);
                    let slug = slug::slugify(&text);
                    let suffix: u32 = rand::rng().random();
                    meta.id = Some(format!("{}-{}", slug, suffix));
                }
            }
            IRNode::Root(children)
            | IRNode::Block { children, .. }
            | IRNode::FlexContainer { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => {
                ensure_heading_ids(children);
            }
            IRNode::Table { header, body, .. } => {
                if let Some(h) = header {
                    for row in &h.rows {
                        for cell in &row.cells {
                            ensure_heading_ids(&mut cell.children.clone());
                        }
                    }
                }
                for row in &body.rows {
                    for cell in &row.cells {
                        ensure_heading_ids(&mut cell.children.clone());
                    }
                }
            }
            _ => {}
        }
    }
}

/// Recursively collects TOC entries from headings that have IDs.
/// Only collects headings with level > 1 to avoid including the main title or "Table of Contents" heading.
fn collect_toc_entries(node: &IRNode, entries: &mut Vec<TocEntry>) {
    match node {
        IRNode::Heading { meta, level, children, } => {
            if *level > 1 { // Only include sub-headings in the TOC
                if let Some(id) = &meta.id {
                    entries.push(TocEntry {
                        level: *level,
                        text: extract_text_from_inlines(children),
                        target_id: id.clone(),
                    });
                }
            }
        }
        IRNode::Root(children)
        | IRNode::Block { children, .. }
        | IRNode::FlexContainer { children, .. }
        | IRNode::List { children, .. }
        | IRNode::ListItem { children, .. } => {
            for child in children {
                collect_toc_entries(child, entries);
            }
        }
        IRNode::Table { header, body, .. } => {
            if let Some(h) = header {
                for row in &h.rows {
                    for cell in &row.cells {
                        collect_toc_entries(&IRNode::Root(cell.children.clone()), entries);
                    }
                }
            }
            for row in &body.rows {
                for cell in &row.cells {
                    collect_toc_entries(&IRNode::Root(cell.children.clone()), entries);
                }
            }
        }
        _ => {}
    }
}


fn extract_text_from_inlines(inlines: &[InlineNode]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            InlineNode::Text(t) => text.push_str(t),
            InlineNode::StyledSpan { children, .. }
            | InlineNode::Hyperlink { children, .. }
            | InlineNode::PageReference { children, .. } => {
                text.push_str(&extract_text_from_inlines(children));
            }
            InlineNode::LineBreak => text.push(' '),
            InlineNode::Image { .. } => {} // Skip images in TOC text
        }
    }
    text
}

/// Traverses the IR tree to find all unique image `src` URIs.
fn collect_image_uris(node: &IRNode, uris: &mut HashSet<String>) {
    match node {
        IRNode::Image { meta: _, src } => {
            uris.insert(src.clone());
        }
        IRNode::Paragraph { children, .. } | IRNode::Heading { children, .. } => {
            for inline in children {
                collect_inline_image_uris(inline, uris);
            }
        }
        IRNode::Root(children)
        | IRNode::Block { children, .. }
        | IRNode::FlexContainer { children, .. }
        | IRNode::List { children, .. }
        | IRNode::ListItem { children, .. } => {
            for child in children {
                collect_image_uris(child, uris);
            }
        }
        IRNode::Table { header, body, .. } => {
            if let Some(h) = header {
                for row in &h.rows {
                    for cell in &row.cells {
                        for child in &cell.children {
                            collect_image_uris(child, uris);
                        }
                    }
                }
            }
            for row in &body.rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        collect_image_uris(child, uris);
                    }
                }
            }
        }
        IRNode::PageBreak { .. } | IRNode::TableOfContents { .. } => {}
    }
}

/// A recursive helper to find image URIs in inline elements.
fn collect_inline_image_uris(inline: &InlineNode, uris: &mut HashSet<String>) {
    match inline {
        InlineNode::Image { src, .. } => {
            uris.insert(src.clone());
        }
        InlineNode::StyledSpan { children, .. }
        | InlineNode::Hyperlink { children, .. }
        | InlineNode::PageReference { children, .. } => {
            for child in children {
                collect_inline_image_uris(child, uris);
            }
        }
        _ => {}
    }
}

/// Gathers all unique image URIs from a tree and loads them from disk.
fn collect_and_load_resources(
    node: &IRNode,
    base_path: &Path,
) -> Result<HashMap<String, SharedData>, PipelineError> {
    let mut uris = HashSet::new();
    collect_image_uris(node, &mut uris);

    let mut resources = HashMap::new();
    for uri in uris {
        if !uri.is_empty() {
            let image_path = base_path.join(&uri);
            match fs::read(&image_path) {
                Ok(image_bytes) => {
                    resources.insert(uri, Arc::new(image_bytes));
                }
                Err(e) => {
                    log::warn!("Failed to load image resource '{}': {}", image_path.display(), e);
                }
            }
        }
    }
    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::core::idf::NodeMetadata;

    #[test]
    fn test_collect_and_load_resources() {
        let dir = tempdir().unwrap();
        let img_path = dir.path().join("test.png");
        fs::write(&img_path, "image_data").unwrap();

        let node = IRNode::Root(vec![IRNode::Image {
            src: "test.png".to_string(),
            meta: NodeMetadata::default(),
        }]);

        let resources = collect_and_load_resources(&node, dir.path()).unwrap();

        assert_eq!(resources.len(), 1);
        let loaded_data = resources.get("test.png").expect("Image data should be loaded");
        assert_eq!(**loaded_data, b"image_data");
    }

    #[test]
    fn test_collect_and_load_skips_empty_src() {
        let node = IRNode::Image {
            src: "".to_string(), // Empty src
            meta: NodeMetadata::default(),
        };
        // Should not panic or error
        let resources = collect_and_load_resources(&node, Path::new("/tmp")).unwrap();
        assert!(resources.is_empty());
    }
}