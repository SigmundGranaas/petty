// src/pipeline/strategy/two_pass.rs
//! Reusable concurrency primitives originally from the `TwoPassStrategy`.
//! These helpers (producer, worker, consumer) form the backbone of both the
//! simple streaming pipeline and the metadata generation pipeline.

use super::PipelineContext;
use crate::core::layout::LayoutEngine;
use crate::error::PipelineError;
use crate::parser::processor::{DataSourceFormat, ExecutionConfig};
use crate::pipeline::worker::{finish_layout_and_resource_loading, LaidOutSequence};
use crate::render::lopdf_helpers;
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::renderer::{HyperlinkLocation, Pass1Result, ResolvedAnchor};
use crate::render::DocumentRenderer;
use async_channel;
use log::{debug, info, warn};
use lopdf::dictionary;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::{Seek, Write};
use std::sync::Arc;
use tokio::task;

pub(crate) async fn producer_task<I>(
    data_iterator: I,
    tx: async_channel::Sender<Result<(usize, Arc<Value>), PipelineError>>,
) where
    I: Iterator<Item = Value> + Send + 'static,
{
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

/// Spawns worker threads for layout tasks.
pub(crate) fn spawn_workers(
    num_threads: usize,
    context: &PipelineContext,
    rx: async_channel::Receiver<Result<(usize, Arc<Value>), PipelineError>>,
    tx: async_channel::Sender<(usize, Result<LaidOutSequence, PipelineError>)>,
) -> Vec<task::JoinHandle<()>> {
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
                        let exec_config = ExecutionConfig {
                            format: DataSourceFormat::Json,
                            strict: false,
                        };
                        let layout_result = template_clone
                            .execute(&data_source_string, exec_config)
                            .and_then(|ir_nodes| {
                                finish_layout_and_resource_loading(
                                    worker_id,
                                    ir_nodes,
                                    context_arc.clone(),
                                    template_clone.resource_base_path(),
                                    &layout_engine_clone,
                                    &template_clone.stylesheet(),
                                    false,
                                )
                            });
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
pub(crate) fn run_in_order_streaming_consumer<W: Write + Seek + Send + 'static>(
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
    let font_map: HashMap<String, String> = renderer
        .layout_engine
        .font_manager
        .db()
        .faces()
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
                    pass1_result.resolved_anchors.insert(
                        name.clone(),
                        ResolvedAnchor {
                            global_page_index: global_page_offset + anchor.local_page_index + 1,
                            y_pos: anchor.y_pos,
                        },
                    );
                }
                for (local_page_idx, page_elements) in seq.pages.iter().enumerate() {
                    let current_global_page_idx = global_page_offset + local_page_idx + 1;
                    for el in page_elements {
                        use crate::core::layout::LayoutElement;
                        let href = match &el.element {
                            LayoutElement::Text(t) => t.href.as_ref(),
                            _ => None,
                        };
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
                if !seq.toc_entries.is_empty() {
                    return Err(PipelineError::Config(
                        "Template uses a Table of Contents, which requires Auto (metadata) mode."
                            .into(),
                    ));
                }
                if seq.pages.iter().flatten().any(|el| {
                    use crate::core::layout::LayoutElement;
                    matches!(el.element, LayoutElement::PageNumberPlaceholder { .. })
                }) {
                    return Err(PipelineError::Config(
                        "Template uses Page-X-of-Y placeholders, which requires Auto (metadata) mode."
                            .into(),
                    ));
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