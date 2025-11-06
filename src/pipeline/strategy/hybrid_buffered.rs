// FILE: src/pipeline/strategy/hybrid_buffered.rs
// src/pipeline/strategy/hybrid_buffered.rs
use super::{PipelineContext};
use crate::error::PipelineError;
use crate::pipeline::config::PdfBackend;
use crate::pipeline::strategy::two_pass::{producer_task, spawn_workers, run_in_order_streaming_consumer};
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::renderer::{Pass1Result};
use log::{info};
use serde_json::Value;
use std::io::{self, Cursor, Seek, SeekFrom, Write};
use tokio::task;
use crate::core::idf::{IRNode, InlineNode, NodeMetadata};
use crate::core::layout::LayoutEngine;
use crate::pipeline::worker::TocEntry;
use crate::render::{lopdf_helpers, DocumentRenderer};
use lopdf::{dictionary, Document, Object, ObjectId, StringFormat, Dictionary};
use std::sync::Arc;
use tempfile;
use std::collections::HashMap;

#[derive(Clone)]
pub struct HybridBufferedStrategy {
    pdf_backend: PdfBackend,
}

impl HybridBufferedStrategy {
    pub fn new(pdf_backend: PdfBackend) -> Self {
        Self { pdf_backend }
    }

    pub fn generate<W, I>(
        &self,
        context: &PipelineContext,
        data_iterator: I,
        mut writer: W,
    ) -> Result<W, PipelineError>
    where
        W: Write + Seek + Send + 'static,
        I: Iterator<Item = Value> + Send + 'static,
    {
        if !matches!(self.pdf_backend, PdfBackend::Lopdf | PdfBackend::LopdfParallel) {
            return Err(PipelineError::Config("HybridBufferedStrategy only supports the 'Lopdf' or 'LopdfParallel' backend.".into()));
        }

        let num_layout_threads = num_cpus::get().saturating_sub(1).max(4);
        let channel_buffer_size = num_layout_threads;

        info!("Starting Hybrid Buffered pipeline with {} layout workers.", num_layout_threads);

        let (tx1, rx1) = async_channel::bounded(channel_buffer_size);
        let (tx2, rx2) = async_channel::bounded(channel_buffer_size);

        let producer = task::spawn(producer_task(data_iterator, tx1));
        let workers = spawn_workers(num_layout_threads, context, rx1, tx2);

        // --- Main Body Pass (Render to Tempfile via In-Order Streaming Consumer) ---
        info!("[HYBRID] Starting main body pass, streaming render to temporary file.");
        let mut temp_file = tempfile::tempfile()?;
        let mut pass1_result: Pass1Result;
        {
            let final_layout_engine = LayoutEngine::new(Arc::clone(&context.font_manager));
            let final_stylesheet = context.compiled_template.stylesheet();

            // For this pass, the renderer's destination is an in-memory buffer (Cursor).
            let mut renderer = LopdfRenderer::new(final_layout_engine, final_stylesheet.clone())?;
            renderer.begin_document(Cursor::new(Vec::new()))?;

            let (page_width, page_height) = renderer.stylesheet.get_default_page_layout().size.dimensions_pt();

            // This consumer processes laid-out pages in strict order, buffering only when necessary.
            let (all_page_ids, p1_result) = run_in_order_streaming_consumer(
                rx2,
                &mut renderer,
                page_width,
                page_height,
                true, // Enable analysis for hybrid mode
            )?;
            pass1_result = p1_result;


            // Use the specialized method to get the bytes from the renderer's `Cursor`.
            let pdf_bytes = renderer.finish_into_buffer(all_page_ids)?;
            temp_file.write_all(&pdf_bytes)?;
            temp_file.flush()?;
        }
        info!(
            "[HYBRID] Main body pass complete. Pass1 Result: total_pages={}, toc_entries={}, resolved_anchors={}, hyperlink_locations={}",
            pass1_result.total_pages,
            pass1_result.toc_entries.len(),
            pass1_result.resolved_anchors.len(),
            pass1_result.hyperlink_locations.len()
        );


        producer.abort();
        for worker in workers { worker.abort(); }

        // FAST PATH: If no ToC or other forward references were found, we can just copy the temp file.
        if pass1_result.toc_entries.is_empty() && pass1_result.hyperlink_locations.is_empty() && !pass1_result.resolved_anchors.values().any(|a| a.global_page_index > pass1_result.total_pages) {
            info!("[HYBRID] No forward references found. Copying temp file directly to output.");
            temp_file.seek(SeekFrom::Start(0))?;
            io::copy(&mut temp_file, &mut writer)?;
            return Ok(writer);
        }

        // --- ToC Generation and Merging ---
        info!("[HYBRID] Generating ToC and merging documents.");
        temp_file.seek(SeekFrom::Start(0))?;
        let mut doc = Document::load_from(&mut temp_file)?;
        info!("[HYBRID] Loaded temporary PDF with {} pages.", doc.get_pages().len());

        let layout_engine = LayoutEngine::new(Arc::clone(&context.font_manager));
        let stylesheet = context.compiled_template.stylesheet();
        let (page_width, page_height) = stylesheet.get_default_page_layout().size.dimensions_pt();

        // Find the main Pages and Resources objects from the loaded doc
        let root_id = doc.trailer.get(b"Root")?.as_reference()?;
        let root_dict = doc.get_object(root_id)?.as_dict()?;
        let pages_id = root_dict.get(b"Pages")?.as_reference()?;

        // The logic to generate and prepend a visible ToC page has been removed.
        // This strategy was flawed as it broke documents where the ToC was not on a dedicated page.
        // The Hybrid strategy will now only handle "invisible" forward references like
        // PDF Outlines (bookmarks) and hyperlinks, which can be fixed up without re-layout.
        // Visible ToC generation requires the TwoPass strategy.
        info!("[HYBRID] Skipping visible ToC generation. Only applying outlines and links.");

        // --- Document Assembly ---
        let body_page_ids: Vec<ObjectId> = doc.get_pages().into_values().collect(); // BTreeMap values are sorted by key.
        let final_page_ids = body_page_ids;
        info!("[HYBRID] Assembling final document. Body pages: {}.", final_page_ids.len());

        // We don't need to modify the Pages dictionary if we aren't adding/removing pages.
        let final_page_count = final_page_ids.len();

        // --- PASS 2: Fixups ---
        info!("[HYBRID] Applying forward-reference fixups (links, outlines).");
        // NOTE: In this simplified strategy, page indices do not need to be adjusted
        // as we are no longer prepending pages.

        let annots_by_page = create_link_annotations_for_doc(&mut doc, &pass1_result, &final_page_ids, page_height)?;
        for (page_idx, annot_ids) in annots_by_page {
            if page_idx > 0 && page_idx <= final_page_count {
                let page_id = final_page_ids[page_idx - 1];
                if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page_id) {
                    let annots_array = annot_ids.into_iter().map(Object::Reference).collect();
                    page_dict.set("Annots", Object::Array(annots_array));
                }
            }
        }
        if let Some(outline_root_id) = build_outlines_for_doc(&mut doc, &pass1_result, &final_page_ids, page_height)? {
            if let Ok(Object::Dictionary(root_dict_mut)) = doc.get_object_mut(root_id) {
                root_dict_mut.set("Outlines", outline_root_id);
                root_dict_mut.set("PageMode", "UseOutlines");
            }
        }

        doc.save_to(&mut writer)?;
        info!("[HYBRID] Merged document saved successfully.");

        Ok(writer)
    }
}


/// Generates a simple list of hyperlinks as `IRNode`s from `TocEntry` data.
fn generate_toc_nodes(toc_entries: &[TocEntry]) -> Vec<IRNode> {
    toc_entries
        .iter()
        .map(|entry| {
            let link = InlineNode::Hyperlink {
                children: vec![InlineNode::Text(entry.text.clone())],
                href: format!("#{}", entry.target_id),
                meta: Default::default(),
            };
            IRNode::Paragraph {
                children: vec![link],
                meta: NodeMetadata::default(),
            }
        })
        .collect()
}


fn create_link_annotations_for_doc(
    doc: &mut Document,
    pass1_result: &Pass1Result,
    final_page_ids: &[ObjectId],
    page_height: f32,
) -> Result<HashMap<usize, Vec<ObjectId>>, PipelineError> {
    let mut annots_by_page: HashMap<usize, Vec<ObjectId>> = HashMap::new();
    for link in &pass1_result.hyperlink_locations {
        if let Some(anchor) = pass1_result.resolved_anchors.get(&link.target_id) {
            if anchor.global_page_index > 0 && anchor.global_page_index <= final_page_ids.len() {
                let target_page_id = final_page_ids[anchor.global_page_index - 1];
                let y_dest = page_height - anchor.y_pos;
                let dest = vec![Object::Reference(target_page_id), "FitH".into(), y_dest.into()];
                let action = dictionary! { "Type" => "Action", "S" => "GoTo", "D" => dest };
                let action_id = doc.add_object(action);
                let rect = vec![
                    link.rect[0].into(), (page_height - link.rect[3]).into(),
                    link.rect[2].into(), (page_height - link.rect[1]).into(),
                ];
                let annot = dictionary! {
                    "Type" => "Annot", "Subtype" => "Link", "Rect" => rect,
                    "Border" => vec![0.into(), 0.into(), 0.into()], "A" => action_id,
                };
                let annot_id = doc.add_object(annot);
                annots_by_page.entry(link.global_page_index).or_default().push(annot_id);
            }
        }
    }
    Ok(annots_by_page)
}

fn build_outlines_for_doc(
    doc: &mut Document,
    pass1_result: &Pass1Result,
    final_page_ids: &[ObjectId],
    page_height: f32,
) -> Result<Option<ObjectId>, PipelineError> {
    if pass1_result.toc_entries.is_empty() { return Ok(None); }

    struct FlatOutlineItem { id: ObjectId, parent_idx: Option<usize>, dict: Dictionary }
    struct NodeOutlineItem { id: ObjectId, children: Vec<NodeOutlineItem>, dict: Dictionary }

    let mut flat_list = Vec::new();
    let mut level_stack: Vec<(u8, usize)> = vec![(0, usize::MAX)];

    for entry in &pass1_result.toc_entries {
        if let Some(anchor) = pass1_result.resolved_anchors.get(&entry.target_id) {
            if anchor.global_page_index == 0 || anchor.global_page_index > final_page_ids.len() { continue; }
            let dest_page_id = final_page_ids[anchor.global_page_index - 1];
            let y_dest = page_height - anchor.y_pos;
            let dest = vec![Object::Reference(dest_page_id), "FitH".into(), y_dest.into()];

            while level_stack.last().unwrap().0 >= entry.level { level_stack.pop(); }
            let parent_idx = if level_stack.last().unwrap().1 == usize::MAX { None } else { Some(level_stack.last().unwrap().1) };

            let dict = dictionary! { "Title" => Object::String(entry.text.as_bytes().to_vec(), StringFormat::Literal), "Dest" => dest };
            let new_item = FlatOutlineItem { id: doc.new_object_id(), parent_idx, dict, };
            let new_idx = flat_list.len();
            flat_list.push(new_item);
            level_stack.push((entry.level, new_idx));
        }
    }

    if flat_list.is_empty() { return Ok(None); }

    let mut children_map: HashMap<usize, Vec<NodeOutlineItem>> = HashMap::new();
    let mut root_items = Vec::new();
    for (i, flat_node) in flat_list.into_iter().enumerate().rev() {
        let mut children = children_map.remove(&i).unwrap_or_default();
        children.reverse();
        let node = NodeOutlineItem { id: flat_node.id, children, dict: flat_node.dict };
        if let Some(parent_idx) = flat_node.parent_idx { children_map.entry(parent_idx).or_default().push(node); }
        else { root_items.push(node); }
    }
    root_items.reverse();
    if root_items.is_empty() { return Ok(None); }

    let outline_root_id = doc.add_object(dictionary! {
        "Type" => "Outlines",
        "First" => Object::Reference(root_items.first().unwrap().id),
        "Last" => Object::Reference(root_items.last().unwrap().id),
        "Count" => root_items.len() as i64,
    });

    fn add_outline_level(items: &[NodeOutlineItem], parent_id: ObjectId, doc: &mut Document) {
        for (i, item) in items.iter().enumerate() {
            let mut dict = item.dict.clone();
            dict.set("Parent", Object::Reference(parent_id));
            if i > 0 { dict.set("Prev", Object::Reference(items[i - 1].id)); }
            if i < items.len() - 1 { dict.set("Next", Object::Reference(items[i + 1].id)); }
            if !item.children.is_empty() {
                dict.set("First", Object::Reference(item.children.first().unwrap().id));
                dict.set("Last", Object::Reference(item.children.last().unwrap().id));
                dict.set("Count", -(item.children.len() as i64));
                add_outline_level(&item.children, item.id, doc);
            }
            doc.objects.insert(item.id, dict.into());
        }
    }
    add_outline_level(&root_items, outline_root_id, doc);
    Ok(Some(outline_root_id))
}