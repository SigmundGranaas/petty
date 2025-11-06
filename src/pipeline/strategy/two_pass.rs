// src/pipeline/strategy/two_pass.rs
use super::{PipelineContext};
use crate::error::PipelineError;
use crate::pipeline::config::PdfBackend;
use crate::pipeline::worker::{finish_layout_and_resource_loading, LaidOutSequence, TocEntry};
use crate::render::lopdf_helpers;
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::renderer::{Pass1Result, RenderError, ResolvedAnchor, HyperlinkLocation};
use crate::render::DocumentRenderer;
use async_channel;
use log::{debug, info, warn};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::{Seek, Write};
use std::sync::Arc;
use tokio::task;
use crate::core::idf::SharedData;
use crate::core::layout::{AnchorLocation, LayoutElement, LayoutEngine, PositionedElement, TextElement};
use crate::parser::processor::{DataSourceFormat, ExecutionConfig};
use lopdf::dictionary;

/// A helper trait for creating a `Box<dyn ...>` that requires multiple traits.
pub trait PdfWrite: Write + Seek + Send {}
impl<T: Write + Seek + Send> PdfWrite for T {}


#[derive(Clone)]
pub struct TwoPassStrategy {
    pdf_backend: PdfBackend,
}

impl TwoPassStrategy {
    pub fn new(pdf_backend: PdfBackend) -> Self {
        Self { pdf_backend }
    }

    pub fn generate<W, I>(
        &self,
        context: &PipelineContext,
        data_iterator: I,
        writer: W,
    ) -> Result<W, PipelineError>
    where
        W: Write + Seek + Send + 'static,
        I: Iterator<Item = Value> + Send + 'static + Clone,
    {
        info!("[CONSUMER] Started Two-Pass Strategy. Awaiting laid-out sequences.");
        let num_layout_threads = num_cpus::get().saturating_sub(1).max(4);

        // --- PASS 1: Analysis ---
        info!("[PASS 1] Starting analysis pass to collect metadata.");
        let pass1_result = {
            let (tx1, rx1) = async_channel::bounded(num_layout_threads);
            let (tx2, rx2) = async_channel::bounded(num_layout_threads);
            let producer = task::spawn(producer_task(data_iterator.clone(), tx1));
            let workers = spawn_workers(num_layout_threads, context, rx1, tx2);

            let pass1_sequences = consume_and_reorder_pass1(rx2)?;
            let meta = build_pass1_result(pass1_sequences);

            producer.abort();
            for worker in workers { worker.abort(); }
            meta
        };
        info!("[PASS 1] Analysis complete. Total pages: {}.", pass1_result.total_pages);

        // --- PASS 2: Generation & Assembly ---
        info!("[PASS 2] Starting generation pass.");
        let (tx1, rx1) = async_channel::bounded(num_layout_threads);
        let (tx2, rx2) = async_channel::bounded(num_layout_threads);
        let producer = task::spawn(producer_task(data_iterator, tx1));
        let workers = spawn_workers(num_layout_threads, context, rx1, tx2);

        let sequences = consume_and_reorder_pass2(rx2)?;

        let final_layout_engine = LayoutEngine::new(Arc::clone(&context.font_manager));
        let final_stylesheet = context.compiled_template.stylesheet();

        if !matches!(self.pdf_backend, PdfBackend::Lopdf | PdfBackend::LopdfParallel) {
            warn!("TwoPassStrategy currently only supports the 'lopdf' backend for full feature support. Other backends may not work correctly.");
        }

        let mut renderer = LopdfRenderer::new(final_layout_engine.clone(), final_stylesheet.clone())?;
        renderer.begin_document(writer)?;

        let (page_width, page_height) = final_stylesheet.get_default_page_layout().size.dimensions_pt();

        let mut page_content_ids = Vec::new();
        let mut fixup_elements_by_page: HashMap<usize, Vec<PositionedElement>> = HashMap::new();
        let mut global_page_idx = 0;
        let font_map: HashMap<String, String> = final_layout_engine.font_manager.db().faces()
            .enumerate()
            .map(|(i, face)| (face.post_script_name.clone(), format!("F{}", i + 1)))
            .collect();

        for seq in &sequences {
            renderer.add_resources(&seq.resources)?;
            for page_elements in &seq.pages {
                for el in page_elements {
                    if let LayoutElement::PageNumberPlaceholder { target_id, href } = &el.element {
                        let page_num_str = pass1_result.resolved_anchors
                            .get(target_id)
                            .map(|anchor| anchor.global_page_index.to_string())
                            .unwrap_or_else(|| "XX".to_string());

                        let text_el = PositionedElement {
                            element: LayoutElement::Text(TextElement { content: page_num_str, href: href.clone(), text_decoration: el.style.text_decoration.clone() }),
                            ..el.clone()
                        };
                        fixup_elements_by_page.entry(global_page_idx).or_default().push(text_el);
                    }
                }
                let content_id = renderer.render_page_content(page_elements.clone(), &font_map, page_width, page_height)?;
                page_content_ids.push(content_id);
                global_page_idx += 1;
            }
        }

        let lopdf_renderer = &mut renderer;

        let (final_page_ids, link_annots_by_page, outline_root_id) = {
            let writer = lopdf_renderer.writer_mut().ok_or_else(|| RenderError::Other("Renderer not initialized".to_string()))?;
            let page_ids: Vec<_> = (0..pass1_result.total_pages).map(|_| writer.new_object_id()).collect();
            let annots = lopdf_helpers::create_link_annotations(writer, &pass1_result, &sequences, &page_ids, page_height)?;
            let outline = lopdf_helpers::build_outlines(writer, &pass1_result, &page_ids, page_height)?;
            (page_ids, annots, outline)
        };
        if let Some(id) = outline_root_id { lopdf_renderer.set_outline_root(id); }

        let mut fixup_content_streams = HashMap::new();
        {
            let writer = lopdf_renderer.writer_mut().ok_or_else(|| RenderError::Other("Renderer not initialized".to_string()))?;
            for (page_idx, elements) in fixup_elements_by_page {
                let content = lopdf_helpers::render_elements_to_content(elements, &font_map, page_width, page_height)?;
                let content_id = writer.buffer_content_stream(content);
                fixup_content_streams.insert(page_idx, content_id);
            }
        }
        for i in 0..pass1_result.total_pages {
            let mut contents = vec![page_content_ids[i]];
            if let Some(fixup_id) = fixup_content_streams.get(&i) { contents.push(*fixup_id); }
            let annots = link_annots_by_page.get(&i).cloned().unwrap_or_default();
            lopdf_renderer.write_page_object_at_id(final_page_ids[i], contents, annots, page_width, page_height)?;
        }

        let final_writer = Box::new(renderer).finish(final_page_ids)?;

        producer.abort();
        for worker in workers { worker.abort(); }
        info!("[CONSUME] Finished.");
        Ok(final_writer)
    }
}


#[derive(Clone, Default)]
struct Pass1Metadata {
    resources: HashMap<String, SharedData>,
    defined_anchors: HashMap<String, AnchorLocation>,
    toc_entries: Vec<TocEntry>,
    page_count: usize,
}
fn consume_and_reorder_pass1(rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>) -> Result<Vec<Pass1Metadata>, PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_idx = 0;
    let mut results = Vec::new();
    while let Ok((index, result)) = rx.recv_blocking() {
        buffer.insert(index, result);
        while let Some(res) = buffer.remove(&next_idx) {
            let seq = res?;
            results.push(Pass1Metadata { resources: seq.resources, defined_anchors: seq.defined_anchors, toc_entries: seq.toc_entries, page_count: seq.pages.len(), });
            next_idx += 1;
        }
    }
    Ok(results)
}
fn build_pass1_result(pass1_sequences: Vec<Pass1Metadata>) -> Pass1Result {
    let mut pass1_result = Pass1Result::default();
    let mut global_page_offset = 0;
    for seq in &pass1_sequences {
        pass1_result.total_pages += seq.page_count;
        pass1_result.toc_entries.extend(seq.toc_entries.clone());
        for (name, anchor) in &seq.defined_anchors {
            pass1_result.resolved_anchors.insert( name.clone(), ResolvedAnchor { global_page_index: global_page_offset + anchor.local_page_index + 1, y_pos: anchor.y_pos, }, );
        }
        global_page_offset += seq.page_count;
    }
    pass1_result
}
pub(super) async fn producer_task<I>(data_iterator: I, tx: async_channel::Sender<Result<(usize, Arc<Value>), PipelineError>>) where I: Iterator<Item = Value> + Send + 'static {
    info!("[PRODUCER] Starting sequence production from iterator.");
    for (i, item) in data_iterator.enumerate() {
        debug!("[PRODUCER] Sending item #{} to layout workers.", i);
        if tx.send(Ok((i, Arc::new(item)))).await.is_err() {
            warn!("[PRODUCER] Layout channel closed, stopping producer.");
            break;
        }
    }
    info!("[PRODUCER] Finished sequence production.");
}
pub(super) fn spawn_workers( num_threads: usize, context: &PipelineContext, rx: async_channel::Receiver<Result<(usize, Arc<Value>), PipelineError>>, tx: async_channel::Sender<(usize, Result<LaidOutSequence, PipelineError>)>, ) -> Vec<task::JoinHandle<()>> {
    let mut handles = Vec::new();
    let layout_engine = LayoutEngine::new(Arc::clone(&context.font_manager));
    for worker_id in 0..num_threads {
        let rx_clone = rx.clone();
        let tx_clone = tx.clone();
        let layout_engine_clone = layout_engine.clone();
        let template_clone = Arc::clone(&context.compiled_template);
        let worker_handle = task::spawn_blocking(move || {
            info!("[WORKER-{}] Started.", worker_id);
            while let Ok(result) = rx_clone.recv_blocking() {
                let (index, work_result) = match result {
                    Ok((index, context_arc)) => {
                        let data_source_string = serde_json::to_string(&*context_arc).unwrap();
                        let exec_config = ExecutionConfig { format: DataSourceFormat::Json, strict: false };
                        let layout_result = template_clone.execute(&data_source_string, exec_config) .and_then(|ir_nodes| { finish_layout_and_resource_loading( worker_id, ir_nodes, context_arc.clone(), template_clone.resource_base_path(), &layout_engine_clone, &template_clone.stylesheet(), false, ) });
                        (index, layout_result)
                    }
                    Err(e) => (0, Err(e)),
                };
                if tx_clone.send_blocking((index, work_result)).is_err() {
                    warn!("[WORKER-{}] Consumer channel closed.", worker_id);
                    break;
                }
            }
            info!("[WORKER-{}] Shutting down.", worker_id);
        });
        handles.push(worker_handle);
    }
    drop(rx);
    drop(tx);
    handles
}

/// A true streaming consumer that guarantees in-order processing.
///
/// It receives laid-out sequences from workers and uses a re-ordering buffer
/// to ensure that `sequence-N` is always processed and written to the stream
/// before `sequence-N+1`. This maintains low memory by only buffering the
/// out-of-order "gaps" in the sequence.
pub(super) fn run_in_order_streaming_consumer<W: Write + Seek + Send + 'static>(
    rx2: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
    renderer: &mut LopdfRenderer<W>,
    page_width: f32,
    page_height: f32,
    perform_analysis: bool,
) -> Result<(Vec<lopdf::ObjectId>, Pass1Result), PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_sequence_idx = 0;
    let mut all_page_ids = Vec::new();
    let mut pass1_result = Pass1Result::default();
    let mut global_page_offset = 0;
    let font_map: HashMap<String, String> = renderer.layout_engine.font_manager.db().faces()
        .enumerate()
        .map(|(i, face)| (face.post_script_name.clone(), format!("F{}", i + 1)))
        .collect();


    while let Ok((index, result)) = rx2.recv_blocking() {
        buffer.insert(index, result);

        // Process any contiguous sequences that are now available
        while let Some(res) = buffer.remove(&next_sequence_idx) {
            let seq = res?;
            debug!("[CONSUMER] Processing in-order sequence #{}", next_sequence_idx);

            if perform_analysis {
                pass1_result.toc_entries.extend(seq.toc_entries.clone());
                for (name, anchor) in &seq.defined_anchors {
                    pass1_result.resolved_anchors.insert(name.clone(), ResolvedAnchor {
                        global_page_index: global_page_offset + anchor.local_page_index + 1,
                        y_pos: anchor.y_pos,
                    });
                }
                for (local_page_idx, page_elements) in seq.pages.iter().enumerate() {
                    let current_global_page_idx = global_page_offset + local_page_idx + 1;
                    for el in page_elements {
                        let href = match &el.element { LayoutElement::Text(t) => t.href.as_ref(), _ => None };
                        if let Some(href_str) = href {
                            if let Some(target_id) = href_str.strip_prefix('#') {
                                pass1_result.hyperlink_locations.push(HyperlinkLocation {
                                    global_page_index: current_global_page_idx,
                                    rect: [el.x, el.y, el.x + el.width, el.y + el.height],
                                    target_id: target_id.to_string(),
                                });
                            }
                        }
                    }
                }
                pass1_result.total_pages += seq.pages.len();
                global_page_offset += seq.pages.len();
            } else {
                if !seq.toc_entries.is_empty() { return Err(PipelineError::Config("Template uses a Table of Contents, which requires TwoPass or Hybrid mode.".into())); }
                if seq.pages.iter().flatten().any(|el| matches!(el.element, LayoutElement::PageNumberPlaceholder {..})) {
                    return Err(PipelineError::Config("Template uses Page-X-of-Y placeholders, which requires TwoPass or Hybrid mode.".into()));
                }
            }

            renderer.add_resources(&seq.resources)?;
            for page_elements in seq.pages {
                let content = lopdf_helpers::render_elements_to_content(
                    page_elements,
                    &font_map,
                    page_width,
                    page_height,
                )?;
                let writer = renderer.writer.as_mut().unwrap();
                let content_id = writer.write_content_stream(content)?;

                let page_dict = dictionary! {
                    "Type" => "Page",
                    "Parent" => writer.pages_id,
                    "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
                    "Contents" => content_id,
                    "Resources" => writer.resources_id,
                };

                let page_id = writer.write_object(page_dict.into())?;
                all_page_ids.push(page_id);
            }
            next_sequence_idx += 1;
        }
    }

    Ok((all_page_ids, pass1_result))
}


/// Consumes all laid-out sequences from workers and buffers them into a Vec.
///
/// **NOTE:** This function is intentionally designed to collect all results in memory.
/// It is only suitable for the `TwoPassStrategy`, which requires the complete set of
/// sequences to resolve document-wide forward references (e.g., creating all link annotations).
/// It MUST NOT be used by streaming strategies.
pub(super) fn consume_and_reorder_pass2( rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>, ) -> Result<Vec<LaidOutSequence>, PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_sequence_idx = 0;
    let mut all_sequences = Vec::new();
    while let Ok((index, result)) = rx.recv_blocking() {
        buffer.insert(index, result);
        while let Some(res) = buffer.remove(&next_sequence_idx) {
            all_sequences.push(res?);
            next_sequence_idx += 1;
        }
    }
    Ok(all_sequences)
}