// src/pipeline/worker.rs

use petty_core::core::idf::{IRNode, InlineNode, SharedData};
use petty_core::core::layout::{IndexEntry, LayoutEngine, LayoutStore};
use petty_core::core::style::stylesheet::Stylesheet;
use petty_core::error::PipelineError;
use log::{debug, info, trace};
use petty_core::traits::ResourceProvider;
use rand::Rng;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

// Re-export from petty-core
pub use petty_core::{LaidOutSequence, TocEntry};

pub(super) fn finish_layout_and_resource_loading(
    worker_id: usize,
    ir_nodes: Vec<IRNode>,
    _context_arc: Arc<Value>,
    resource_provider: &dyn ResourceProvider,
    layout_engine: &mut LayoutEngine,
    stylesheet: &Stylesheet,
    debug_mode: bool,
) -> Result<LaidOutSequence, PipelineError> {
    let total_start = Instant::now();

    let prep_start = Instant::now();
    let mut ir_nodes_with_ids = ir_nodes;
    ensure_heading_ids(&mut ir_nodes_with_ids);
    let tree = IRNode::Root(ir_nodes_with_ids);
    if prep_start.elapsed().as_millis() > 1 {
        trace!("[WORKER-{}] IR Prep took {:?}", worker_id, prep_start.elapsed());
    }

    if debug_mode {
        debug!("[WORKER-{}] IR tree dump:\n{:#?}", worker_id, &tree);
    }

    let resource_start = Instant::now();
    let resources = collect_and_load_resources(&tree, resource_provider)?;
    if resource_start.elapsed().as_millis() > 5 {
        debug!("[WORKER-{}] Resource load took {:?}", worker_id, resource_start.elapsed());
    }

    let mut toc_entries = Vec::new();
    collect_toc_entries(&tree, &mut toc_entries);

    let layout_phase_start = Instant::now();

    // Use LayoutStore for scoped memory management
    let store = LayoutStore::new();
    // Reset stats for clean metrics per sequence
    layout_engine.reset_stats();

    let build_tree_start = Instant::now();
    let root_render_node = layout_engine
        .build_render_tree(&tree, &store)
        .map_err(|e| PipelineError::Layout(e.to_string()))?;
    if build_tree_start.elapsed().as_millis() > 1 {
        trace!("[WORKER-{}] Build Render Tree took {:?}", worker_id, build_tree_start.elapsed());
    }

    let iterator = layout_engine
        .paginate(stylesheet, root_render_node, &store)
        .map_err(|e| PipelineError::Layout(e.to_string()))?;

    let mut pages = Vec::new();
    let mut defined_anchors = HashMap::new();
    let mut index_entries: HashMap<String, Vec<IndexEntry>> = HashMap::new();

    for page_res in iterator {
        let page = page_res.map_err(|e| PipelineError::Layout(e.to_string()))?;
        pages.push(page.elements);
        defined_anchors.extend(page.anchors);
        for (k, v) in page.index_entries {
            index_entries.entry(k).or_default().extend(v);
        }
    }

    let layout_total = layout_phase_start.elapsed();
    let pages_count = pages.len();
    if layout_total.as_millis() > 50 {
        debug!(
            "[WORKER-{}] Layout total: {:?} for {} pages ({:?}/page avg)",
            worker_id, layout_total, pages_count, layout_total.checked_div(pages_count as u32).unwrap_or(layout_total)
        );
        // FIX: Dump stats when layout is slow to identify bottleneck
        layout_engine.dump_stats(worker_id);
    }

    let total_dur = total_start.elapsed();
    if total_dur.as_millis() > 50 {
        info!("[WORKER-{}] Total Sequence Time: {:?} (Layout: {:?})", worker_id, total_dur, layout_total);
    }

    Ok(LaidOutSequence {
        pages,
        resources,
        defined_anchors,
        toc_entries,
        index_entries,
    })
}

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

fn collect_toc_entries(node: &IRNode, entries: &mut Vec<TocEntry>) {
    match node {
        IRNode::Heading { meta, level, children, .. } => {
            if *level > 0 {
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
                        for child in &cell.children {
                            collect_toc_entries(child, entries);
                        }
                    }
                }
            }
            for row in &body.rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        collect_toc_entries(child, entries);
                    }
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
            InlineNode::Image { .. } => {}
        }
    }
    text
}

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
        _ => {}
    }
}

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

fn collect_and_load_resources(
    node: &IRNode,
    provider: &dyn ResourceProvider,
) -> Result<HashMap<String, SharedData>, PipelineError> {
    let mut uris = HashSet::new();
    collect_image_uris(node, &mut uris);

    let mut resources = HashMap::new();
    for uri in uris {
        if !uri.is_empty() {
            match provider.load(&uri) {
                Ok(data) => {
                    resources.insert(uri, data);
                }
                Err(e) => {
                    log::warn!("Failed to load image resource '{}': {}", uri, e);
                }
            }
        }
    }
    Ok(resources)
}