use crate::error::PipelineError;
use log::{debug, info};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use crate::core::idf::{IRNode, InlineNode, LayoutUnit, SharedData};
use crate::core::layout::{LayoutEngine, PositionedElement};

/// Represents the output of a single worker task: the original data context,
/// the resulting pages of positioned elements, and all loaded resources.
pub(super) struct LaidOutSequence {
    pub context: Arc<Value>,
    pub pages: Vec<Vec<PositionedElement>>,
    pub resources: HashMap<String, SharedData>,
}

/// The second half of a worker's job: takes a parsed IR tree and performs
/// resource loading and layout. This part is generic over the template language.
pub(super) fn finish_layout_and_resource_loading(
    worker_id: usize,
    ir_nodes: Vec<IRNode>,
    context_arc: Arc<Value>,
    resource_base_path: &Path,
    layout_engine: &LayoutEngine,
) -> Result<LaidOutSequence, PipelineError> {
    let total_start = Instant::now();
    let tree = IRNode::Root(ir_nodes);

    let resource_start = Instant::now();
    debug!(
        "[WORKER-{}] Collecting and loading resources relative to '{}'.",
        worker_id,
        resource_base_path.display()
    );
    let resources = collect_and_load_resources(&tree, resource_base_path)?;
    debug!(
        "[WORKER-{}] Finished loading {} resources in {:.2?}.",
        worker_id,
        resources.len(),
        resource_start.elapsed()
    );

    let layout_unit = LayoutUnit {
        tree,
        context: Arc::clone(&context_arc),
    };

    let layout_start = Instant::now();
    debug!("[WORKER-{}] Paginating sequence tree.", worker_id);
    let pages: Vec<Vec<PositionedElement>> = layout_engine.paginate_tree(layout_unit)?.collect();
    debug!(
        "[WORKER-{}] Finished paginating sequence ({} pages) in {:.2?}.",
        worker_id,
        pages.len(),
        layout_start.elapsed()
    );

    info!(
        "[WORKER-{}] Finished processing sequence in {:.2?}.",
        worker_id,
        total_start.elapsed()
    );

    Ok(LaidOutSequence {
        context: context_arc,
        pages,
        resources,
    })
}

/// Traverses the IR tree to find all unique image `src` URIs.
fn collect_image_uris(node: &IRNode, uris: &mut HashSet<String>) {
    match node {
        IRNode::Image { src, .. } => {
            uris.insert(src.clone());
        }
        IRNode::Paragraph { children, .. } => {
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
    }
}

/// A recursive helper to find image URIs in inline elements.
fn collect_inline_image_uris(inline: &InlineNode, uris: &mut HashSet<String>) {
    match inline {
        InlineNode::Image { src, .. } => {
            uris.insert(src.clone());
        }
        InlineNode::StyledSpan { children, .. } | InlineNode::Hyperlink { children, .. } => {
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
                    log::warn!(
                        "Failed to load image resource '{}': {}",
                        image_path.display(),
                        e
                    );
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
            style_sets: vec![],
            style_override: None,
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
            style_sets: vec![],
            style_override: None,
        };
        // Should not panic or error
        let resources = collect_and_load_resources(&node, Path::new("/tmp")).unwrap();
        assert!(resources.is_empty());
    }
}