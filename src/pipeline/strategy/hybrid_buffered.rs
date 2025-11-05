// src/pipeline/strategy/hybrid_buffered.rs
// src/pipeline/strategy/hybrid_buffered.rs
use super::{PipelineContext};
use crate::error::PipelineError;
use crate::pipeline::config::PdfBackend;
use crate::pipeline::strategy::two_pass::{producer_task, spawn_workers, run_in_order_streaming_consumer};
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::renderer::{Pass1Result};
use log::{info, warn};
use serde_json::Value;
use std::io::{self, Cursor, Seek, SeekFrom, Write};
use tokio::task;
use crate::core::idf::{IRNode, InlineNode, NodeMetadata};
use crate::core::layout::LayoutEngine;
use crate::pipeline::worker::TocEntry;
use crate::render::{lopdf_helpers, DocumentRenderer};
use lopdf::{dictionary, Document, Object};
use std::sync::Arc;
use tempfile;

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
        info!("[HYBRID] Main body pass complete.");

        producer.abort();
        for worker in workers { worker.abort(); }

        if pass1_result.toc_entries.is_empty() && !pass1_result.resolved_anchors.values().any(|a| a.global_page_index > pass1_result.total_pages) {
            info!("[HYBRID] No forward references found. Copying temp file directly to output.");
            temp_file.seek(SeekFrom::Start(0))?;
            io::copy(&mut temp_file, &mut writer)?;
            return Ok(writer);
        }

        // --- ToC Generation and Merging ---
        info!("[HYBRID] Generating ToC and merging documents.");
        temp_file.seek(SeekFrom::Start(0))?;
        let mut doc = Document::load_from(&mut temp_file)?;

        let layout_engine = LayoutEngine::new(Arc::clone(&context.font_manager));
        let stylesheet = context.compiled_template.stylesheet();
        let (page_width, page_height) = stylesheet.get_default_page_layout().size.dimensions_pt();

        let toc_nodes = generate_toc_nodes(&pass1_result.toc_entries);
        let (toc_pages, _) = layout_engine.paginate(&stylesheet, toc_nodes)?;
        let num_toc_pages = toc_pages.len();

        let mut toc_page_ids = Vec::new();
        for page_elements in toc_pages {
            let content = lopdf_helpers::render_elements_to_content(page_elements, &layout_engine, &stylesheet, page_width, page_height)?;
            let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(dictionary!{}, content.encode()?)));
            let page_id = doc.add_object(dictionary! {
                 "Type" => "Page",
                 "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
                 "Contents" => content_id,
             });
            toc_page_ids.push(page_id);
        }

        // --- Document Assembly ---
        doc.prune_objects();
        let mut final_page_ids: Vec<lopdf::ObjectId> = toc_page_ids;
        let original_body_pages = doc.get_pages();
        final_page_ids.extend(original_body_pages.values().copied());

        // Find the root Pages dictionary to update it
        let root_id = doc.trailer.get(b"Root")
            .ok()
            .and_then(|obj| obj.as_reference().ok())
            .ok_or_else(|| PipelineError::Other("Could not find Root object in temporary PDF.".into()))?;

        let root_dict = doc.get_object(root_id)
            .map_err(|e| PipelineError::Other(e.to_string()))?
            .as_dict()
            .map_err(|e| PipelineError::Other(e.to_string()))?;

        let pages_id = root_dict.get(b"Pages")
            .ok()
            .and_then(|obj| obj.as_reference().ok())
            .ok_or_else(|| PipelineError::Other("Could not find Pages object in temporary PDF.".into()))?;


        if let Ok(Object::Dictionary(pages_dict)) = doc.get_object_mut(pages_id) {
            let final_page_refs: Vec<Object> = final_page_ids.into_iter()
                .map(Object::Reference)
                .collect();
            pages_dict.set("Kids", Object::Array(final_page_refs));
            pages_dict.set("Count", lopdf::Object::Integer(pages_dict.get(b"Kids").and_then(|o| o.as_array()).map(|a| a.len() as i64).unwrap_or(0)));
        } else {
            return Err(PipelineError::Other("Root Pages object is not a dictionary.".into()));
        }

        for anchor in pass1_result.resolved_anchors.values_mut() {
            anchor.global_page_index += num_toc_pages;
        }

        warn!("[HYBRID] Linking of outlines and annotations is not yet supported in Hybrid mode.");

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
            // Each TOC entry is a paragraph containing one link.
            IRNode::Paragraph {
                children: vec![link],
                meta: NodeMetadata::default(),
            }
        })
        .collect()
}