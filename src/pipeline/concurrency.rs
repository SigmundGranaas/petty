// src/pipeline/concurrency.rs

use petty_core::core::layout::LayoutEngine;
use petty_core::error::PipelineError;
use crate::executor::Executor;
use petty_core::parser::processor::{DataSourceFormat, ExecutionConfig};
use crate::pipeline::context::PipelineContext;
use crate::pipeline::worker::{finish_layout_and_resource_loading, LaidOutSequence};
use petty_core::render::lopdf_helpers;
use petty_core::render::lopdf_renderer::LopdfRenderer;
use petty_core::render::renderer::{HyperlinkLocation, Pass1Result, ResolvedAnchor};
use petty_core::render::DocumentRenderer;
use crate::source::DataSource;
use async_channel;
use log::{debug, info, warn};
use lopdf::dictionary;
use petty_core::ApiIndexEntry;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::{Seek, Write};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tokio::task;

pub(crate) async fn producer_task<I>(
    data_iterator: I,
    tx: async_channel::Sender<Result<(usize, Arc<Value>), PipelineError>>,
    semaphore: Arc<Semaphore>,
) where
    I: Iterator<Item = Value> + Send + 'static,
{
    info!("[PRODUCER] Starting sequence production from iterator.");
    for (i, item) in data_iterator.enumerate() {
        if let Ok(permit) = semaphore.acquire().await {
            permit.forget();
        }

        if i % 100 == 0 {
            debug!("[PRODUCER] Sending item #{}...", i);
        }
        if tx.send(Ok((i, Arc::new(item)))).await.is_err() {
            warn!("[PRODUCER] Layout channel closed, stopping producer.");
            break;
        }
    }
    info!("[PRODUCER] Finished sequence production.");
}

pub(crate) fn spawn_workers(
    num_threads: usize,
    context: &PipelineContext,
    rx: async_channel::Receiver<Result<(usize, Arc<Value>), PipelineError>>,
    tx: async_channel::Sender<(usize, Result<LaidOutSequence, PipelineError>)>,
) -> Vec<task::JoinHandle<()>> {
    let mut handles = Vec::new();
    let cache_config = context.cache_config;

    for worker_id in 0..num_threads {
        let rx_clone = rx.clone();
        let tx_clone = tx.clone();

        // FIX: Use the shared font library from the context.
        // This ensures all workers share the same font database and cache,
        // avoiding repeated I/O and cache misses.
        let current_font_lib = context.font_library.clone();

        let template_clone = Arc::clone(&context.compiled_template);
        let resource_provider_clone = Arc::clone(&context.resource_provider);

        let worker_handle = task::spawn_blocking(move || {
            info!("[WORKER-{}] Started with shared font library and resource provider.", worker_id);

            let mut layout_engine = LayoutEngine::new(&current_font_lib, cache_config);

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
                                    resource_provider_clone.as_ref(),
                                    &mut layout_engine,
                                    &template_clone.stylesheet(),
                                    false,
                                )
                            });

                        if let Ok(seq) = &layout_result {
                            let size = seq.rough_heap_size();
                            if size > 2 * 1024 * 1024 {
                                warn!("[WORKER-{}] LARGE ITEM #{}: ~{:.2} MB", worker_id, index, size as f64 / 1_000_000.0);
                            }
                        }

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

pub(crate) fn run_in_order_streaming_consumer<W: Write + Seek + Send + 'static>(
    rx2: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
    renderer: &mut LopdfRenderer<W>,
    page_width: f32,
    page_height: f32,
    perform_analysis: bool,
    semaphore: Arc<Semaphore>,
) -> Result<(Vec<lopdf::ObjectId>, Pass1Result), PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_sequence_idx = 0;
    let mut all_page_ids = Vec::new();
    let mut pass1_result = Pass1Result::default();
    let mut global_page_offset = 0;

    // Use registered_fonts() to get all fonts from both fontdb and FontProvider
    let font_map: HashMap<String, String> = renderer
        .layout_engine
        .registered_fonts()
        .iter()
        .enumerate()
        .map(|(i, font_info)| (font_info.postscript_name.clone(), format!("F{}", i + 1)))
        .collect();

    let mut last_processed_time = Instant::now();

    while let Ok((index, result)) = rx2.recv_blocking() {
        let wait_time = last_processed_time.elapsed();
        if wait_time.as_millis() > 100 {
            debug!("[CONSUMER] Waited {:?} for sequence #{}", wait_time, index);
        }

        buffer.insert(index, result);

        if buffer.len() > 20 {
            debug!("[CONSUMER] Buffer growing: {} items waiting. Looking for #{}.", buffer.len(), next_sequence_idx);
        }

        while let Some(res) = buffer.remove(&next_sequence_idx) {
            let _process_start = Instant::now();
            let seq = res?;

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
                for (term, locations) in &seq.index_entries {
                    for loc in locations {
                        pass1_result.index_entries.push(ApiIndexEntry {
                            text: term.clone(),
                            page_number: global_page_offset + loc.local_page_index + 1,
                        });
                    }
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
                            log::debug!("[HYPERLINK DETECTION] Found text with href: '{}'", href_str);
                            if let Some(target_id) = href_str.strip_prefix('#') {
                                log::debug!("[HYPERLINK DETECTION] Adding internal link to target: '{}'", target_id);
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
                if !seq.toc_entries.is_empty() || !seq.index_entries.is_empty() {
                    return Err(PipelineError::Config(
                        "Template uses advanced features (ToC/Index) which require Auto (metadata) mode."
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
                let writer = renderer.writer_mut().unwrap();
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

            semaphore.add_permits(1);
            next_sequence_idx += 1;
            last_processed_time = Instant::now();
        }
    }

    Ok((all_page_ids, pass1_result))
}

/// Process a batch of data items using the injected Executor trait.
///
/// This is a simpler, synchronous alternative to the async producer/worker/consumer pattern.
/// It uses the Executor from the PipelineContext to process all items in parallel,
/// then returns the results in order.
///
/// # Arguments
///
/// * `context` - Pipeline context containing the executor, template, and shared resources
/// * `data_items` - Vector of JSON data items to process
///
/// # Returns
///
/// A vector of `LaidOutSequence` results, one for each input item, in the same order.
pub(crate) fn process_batch_with_executor(
    context: &PipelineContext,
    data_items: Vec<Value>,
) -> Vec<Result<LaidOutSequence, PipelineError>> {
    let parallelism = context.executor.parallelism();
    info!(
        "[EXECUTOR] Processing {} items with parallelism level {}",
        data_items.len(),
        parallelism
    );

    // Wrap each data item with its index to preserve order
    let indexed_items: Vec<(usize, Value)> = data_items
        .into_iter()
        .enumerate()
        .collect();

    // Clone Arc references that will be moved into the closure
    let template_clone = Arc::clone(&context.compiled_template);
    let font_library_clone = Arc::clone(&context.font_library);
    let resource_provider_clone = Arc::clone(&context.resource_provider);
    let cache_config = context.cache_config;

    // Process all items in parallel using the executor
    let results = context.executor.execute_all(
        indexed_items,
        move |(worker_id, data_item)| -> Result<LaidOutSequence, PipelineError> {
            // Each worker gets its own layout engine instance
            let mut layout_engine = LayoutEngine::new(&font_library_clone, cache_config);

            let data_source_string = serde_json::to_string(&data_item)?;

            let exec_config = ExecutionConfig {
                format: DataSourceFormat::Json,
                strict: false,
            };

            let ir_nodes = template_clone.execute(&data_source_string, exec_config)?;

            let context_arc = Arc::new(data_item);
            finish_layout_and_resource_loading(
                worker_id,
                ir_nodes,
                context_arc,
                resource_provider_clone.as_ref(),
                &mut layout_engine,
                &template_clone.stylesheet(),
                false,
            )
        },
    );

    info!(
        "[EXECUTOR] Completed processing {} items ({} succeeded, {} failed)",
        results.len(),
        results.iter().filter(|r| r.is_ok()).count(),
        results.iter().filter(|r| r.is_err()).count()
    );

    results
}

/// Process a DataSource using the injected Executor trait.
///
/// This is similar to `process_batch_with_executor` but works with the DataSource
/// abstraction, allowing for more flexible data input (iterators, channels, etc.).
///
/// # Arguments
///
/// * `context` - Pipeline context containing the executor, template, and shared resources
/// * `source` - A DataSource that provides JSON data items
///
/// # Returns
///
/// A vector of `LaidOutSequence` results, one for each input item, in the same order.
pub fn process_data_source_with_executor(
    context: &PipelineContext,
    mut source: impl DataSource,
) -> Vec<Result<LaidOutSequence, PipelineError>> {
    // Collect all items from the source
    let mut data_items = Vec::new();
    while let Some(item) = source.next() {
        data_items.push(item);
    }

    // Use the existing batch processing function
    process_batch_with_executor(context, data_items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::fonts::SharedFontLibrary;
    use crate::executor::ExecutorImpl;
    use crate::parser::json::processor::JsonParser;
    use crate::parser::processor::TemplateParser;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_batch_processing_with_executor() {
        let _ = env_logger::builder().is_test(true).try_init();

        // Create a simple template
        let template_json = json!({
            "_stylesheet": {
                "defaultPageMaster": "default",
                "pageMasters": {
                    "default": { "size": "A4", "margins": "1cm" }
                },
                "styles": {
                    "default": { "font-family": "Helvetica" }
                }
            },
            "_template": {
                "type": "Paragraph",
                "children": [
                    { "type": "Text", "content": "Item {{id}}: {{name}}" }
                ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();

        let parser = JsonParser;
        let features = parser.parse(&template_str, PathBuf::new()).unwrap();

        let library = SharedFontLibrary::new();
        library.load_fallback_font();

        // Create context with sync executor for deterministic testing
        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(features.role_templates),
            font_library: Arc::new(library),
            resource_provider: Arc::new(crate::resource::InMemoryResourceProvider::new()),
            executor: ExecutorImpl::Sync(crate::executor::SyncExecutor::new()),
            cache_config: Default::default(),
        };

        // Create test data
        let data = vec![
            json!({"id": 1, "name": "First"}),
            json!({"id": 2, "name": "Second"}),
            json!({"id": 3, "name": "Third"}),
        ];

        // Process batch
        let results = process_batch_with_executor(&context, data);

        // Verify results
        assert_eq!(results.len(), 3, "Should process all 3 items");
        for (i, result) in results.iter().enumerate() {
            assert!(
                result.is_ok(),
                "Item {} should succeed: {:?}",
                i,
                result.as_ref().err()
            );
            if let Ok(seq) = result {
                assert!(
                    !seq.pages.is_empty(),
                    "Item {} should produce at least one page",
                    i
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "rayon-executor")]
    fn test_batch_processing_with_rayon_executor() {
        let _ = env_logger::builder().is_test(true).try_init();

        // Same setup as above but with Rayon executor
        let template_json = json!({
            "_stylesheet": {
                "defaultPageMaster": "default",
                "pageMasters": {
                    "default": { "size": "A4", "margins": "1cm" }
                },
                "styles": {
                    "default": { "font-family": "Helvetica" }
                }
            },
            "_template": {
                "type": "Paragraph",
                "children": [
                    { "type": "Text", "content": "Item {{id}}" }
                ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();

        let parser = JsonParser;
        let features = parser.parse(&template_str, PathBuf::new()).unwrap();

        let library = SharedFontLibrary::new();
        library.load_fallback_font();

        // Use Rayon executor for parallel processing
        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(features.role_templates),
            font_library: Arc::new(library),
            resource_provider: Arc::new(crate::resource::InMemoryResourceProvider::new()),
            executor: ExecutorImpl::Rayon(crate::executor::RayonExecutor::new()),
            cache_config: Default::default(),
        };

        // Create more data items to test parallel processing
        let data: Vec<Value> = (1..=10)
            .map(|i| json!({"id": i}))
            .collect();

        // Process batch in parallel
        let results = process_batch_with_executor(&context, data);

        // Verify all succeeded
        assert_eq!(results.len(), 10);
        assert!(
            results.iter().all(|r| r.is_ok()),
            "All items should process successfully"
        );
    }

    #[test]
    fn test_data_source_processing() {
        let _ = env_logger::builder().is_test(true).try_init();

        // Create a simple template
        let template_json = json!({
            "_stylesheet": {
                "defaultPageMaster": "default",
                "pageMasters": {
                    "default": { "size": "A4", "margins": "1cm" }
                },
                "styles": {
                    "default": { "font-family": "Helvetica" }
                }
            },
            "_template": {
                "type": "Paragraph",
                "children": [
                    { "type": "Text", "content": "Value: {{value}}" }
                ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();

        let parser = JsonParser;
        let features = parser.parse(&template_str, PathBuf::new()).unwrap();

        let library = SharedFontLibrary::new();
        library.load_fallback_font();

        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(features.role_templates),
            font_library: Arc::new(library),
            resource_provider: Arc::new(crate::resource::InMemoryResourceProvider::new()),
            executor: ExecutorImpl::Sync(crate::executor::SyncExecutor::new()),
            cache_config: Default::default(),
        };

        // Create a VecDataSource
        let data = vec![
            json!({"value": "A"}),
            json!({"value": "B"}),
            json!({"value": "C"}),
        ];
        let source = crate::source::VecDataSource::new(data);

        // Process using DataSource API
        let results = process_data_source_with_executor(&context, source);

        // Verify results
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
    }
}