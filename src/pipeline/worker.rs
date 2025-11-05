use crate::core::idf::{IRNode, InlineMetadata, InlineNode, NodeMetadata, SharedData};
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
pub struct LaidOutSequence {
    pub context: Arc<Value>,
    pub pages: Vec<Vec<PositionedElement>>,
    pub resources: HashMap<String, SharedData>,
    pub defined_anchors: HashMap<String, AnchorLocation>,
    pub toc_entries: Vec<TocEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct TocEntry {
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

    // --- TOC Generation Pass ---
    // Before layout, we must replace any TOC placeholders with actual generated content.
    // This ensures the layout engine receives concrete nodes (paragraphs, hyperlinks)
    // which it already knows how to process.
    let final_ir_nodes = generate_toc_content(ir_nodes_with_ids);

    let tree = IRNode::Root(final_ir_nodes.clone()); // TODO: Avoid clone

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
    let (pages, defined_anchors) = layout_engine.paginate(stylesheet, final_ir_nodes)?;
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

// --- In-Document Table of Contents Generation ---

/// A temporary struct to hold information about headings found in the document.
#[derive(Debug, Clone)]
struct HeadingInfo {
    id: String,
    text: String,
    level: u8,
}

/// Scans the IR tree and replaces any `TableOfContents` nodes with generated content.
/// This acts as a pre-processing pass before layouting.
fn generate_toc_content(nodes: Vec<IRNode>) -> Vec<IRNode> {
    let mut headings = Vec::new();
    collect_headings_recursive(&nodes, &mut headings);

    // If there are no headings, there's nothing to list in the TOC.
    // Return the original nodes to avoid replacing the TOC placeholder with nothing.
    if headings.is_empty() {
        return nodes;
    }

    replace_toc_nodes_recursive(nodes, &headings)
}

/// Recursively traverses the IR tree to find all headings with IDs.
fn collect_headings_recursive(nodes: &[IRNode], headings: &mut Vec<HeadingInfo>) {
    for node in nodes {
        match node {
            IRNode::Heading { meta, level, children, .. } => {
                // Only include headings that are not the main title (level > 1) and have an ID.
                if *level > 1 {
                    if let Some(id) = &meta.id {
                        headings.push(HeadingInfo {
                            id: id.clone(),
                            text: extract_text_from_inlines(children),
                            level: *level,
                        });
                    }
                }
            }
            // Recurse into any node that can contain other nodes.
            IRNode::Root(children)
            | IRNode::Block { children, .. }
            | IRNode::FlexContainer { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => {
                collect_headings_recursive(children, headings);
            }
            IRNode::Table { header, body, .. } => {
                if let Some(h) = header {
                    for row in &h.rows {
                        for cell in &row.cells {
                            collect_headings_recursive(&cell.children, headings);
                        }
                    }
                }
                for row in &body.rows {
                    for cell in &row.cells {
                        collect_headings_recursive(&cell.children, headings);
                    }
                }
            }
            _ => {} // Leaf nodes
        }
    }
}

/// Recursively traverses the IR tree, replacing `TableOfContents` placeholder nodes
/// with a block of generated paragraphs and hyperlinks.
fn replace_toc_nodes_recursive(nodes: Vec<IRNode>, headings: &[HeadingInfo]) -> Vec<IRNode> {
    nodes
        .into_iter()
        .flat_map(|node| -> Vec<IRNode> {
            match node {
                IRNode::TableOfContents { meta } => {
                    let toc_lines: Vec<IRNode> = headings
                        .iter()
                        .map(|heading| {
                            // This is the crucial part of the fix: create a Hyperlink node.
                            let link = InlineNode::Hyperlink {
                                children: vec![InlineNode::Text(heading.text.clone())],
                                href: format!("#{}", heading.id),
                                meta: InlineMetadata::default(),
                            };
                            // Each TOC entry is a paragraph containing one link.
                            IRNode::Paragraph {
                                children: vec![link],
                                meta: NodeMetadata::default(), // Further styling can be added here.
                            }
                        })
                        .collect();
                    // Wrap the generated lines in a single Block node so styles from the
                    // original <toc> element can be applied to the whole group.
                    vec![IRNode::Block { children: toc_lines, meta }]
                }
                // --- Recurse into nodes that have children, rebuilding them ---
                IRNode::Root(children) => {
                    vec![IRNode::Root(replace_toc_nodes_recursive(children, headings))]
                }
                IRNode::Block { children, meta } => {
                    vec![IRNode::Block {
                        children: replace_toc_nodes_recursive(children, headings),
                        meta,
                    }]
                }
                IRNode::FlexContainer { children, meta } => {
                    vec![IRNode::FlexContainer {
                        children: replace_toc_nodes_recursive(children, headings),
                        meta,
                    }]
                }
                IRNode::List { children, meta, start } => {
                    vec![IRNode::List {
                        children: replace_toc_nodes_recursive(children, headings),
                        meta,
                        start,
                    }]
                }
                IRNode::ListItem { children, meta } => {
                    vec![IRNode::ListItem {
                        children: replace_toc_nodes_recursive(children, headings),
                        meta,
                    }]
                }
                IRNode::Table { header, body, meta, columns } => {
                    let new_header = header.map(|mut h| {
                        for row in &mut h.rows {
                            for cell in &mut row.cells {
                                let new_children = replace_toc_nodes_recursive(std::mem::take(&mut cell.children), headings);
                                cell.children = new_children;
                            }
                        }
                        h
                    });
                    let mut new_body = body;
                    for row in &mut new_body.rows {
                        for cell in &mut row.cells {
                            let new_children = replace_toc_nodes_recursive(std::mem::take(&mut cell.children), headings);
                            cell.children = new_children;
                        }
                    }
                    vec![IRNode::Table { header: new_header, body: new_body, meta, columns }]
                }
                // --- Return leaf nodes (or nodes not recursed into) as is ---
                leaf_node => vec![leaf_node],
            }
        })
        .collect()
}


/// Recursively ensures that all headings have a unique ID for anchor generation.
fn ensure_heading_ids(nodes: &mut [IRNode]) {
    for node in nodes {
        match node {
            IRNode::Heading { meta, children, .. } => {
                if meta.id.is_none() {
                    let text = extract_text_from_inlines(children);
                    let slug = slug::slugify(&text);
                    let mut rng = rand::rng();
                    let suffix: u32 = rng.random();
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
                    for row in &mut h.rows {
                        for cell in &mut row.cells {
                            ensure_heading_ids(&mut cell.children);
                        }
                    }
                }
                for row in &mut body.rows {
                    for cell in &mut row.cells {
                        ensure_heading_ids(&mut cell.children);
                    }
                }
            }
            _ => {}
        }
    }
}


/// Recursively collects TOC entries from headings that have IDs.
/// This is used for generating PDF Outlines (Bookmarks), not the in-document TOC.
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