// src/pipeline/orchestrator.rs
use super::config::PdfBackend;
use super::worker::{finish_layout_and_resource_loading, LaidOutSequence};
use crate::core::layout::{FontManager, LayoutEngine, LayoutElement, PositionedElement, TextElement};
use crate::error::PipelineError;
use crate::parser::processor::{CompiledTemplate, DataSourceFormat, ExecutionConfig};
use crate::render::lopdf_helpers;
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::pdf::PdfDocumentRenderer;
use crate::render::renderer::{Pass1Result, ResolvedAnchor};
use crate::render::DocumentRenderer;
use async_channel;
use handlebars::{
    no_escape, Context, Handlebars, Helper, HelperResult, Output,
    RenderContext as HandlebarsRenderContext,
};
use log::{debug, info, warn};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio::task;
use crate::core::style::stylesheet::PageLayout;

/// The main document generation pipeline.
pub struct DocumentPipeline {
    template_engine: Handlebars<'static>,
    compiled_template: Arc<dyn CompiledTemplate>,
    pdf_backend: PdfBackend,
    font_manager: Arc<FontManager>,
    debug: bool,
}

fn format_currency_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut HandlebarsRenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_f64()).unwrap_or(0.0);
    let formatted = format!("{:.2}", param);
    out.write(&formatted)?;
    Ok(())
}

impl DocumentPipeline {
    pub fn new(
        compiled_template: Arc<dyn CompiledTemplate>,
        pdf_backend: PdfBackend,
        font_manager: Arc<FontManager>,
        debug: bool,
    ) -> Self {
        let mut template_engine = Handlebars::new();
        template_engine.set_strict_mode(false);
        template_engine.register_helper("formatCurrency", Box::new(format_currency_helper));
        template_engine.register_escape_fn(no_escape);

        DocumentPipeline {
            template_engine,
            compiled_template,
            pdf_backend,
            font_manager,
            debug,
        }
    }

    pub async fn generate_to_writer_async<W, I>(
        &self,
        data_iterator: I,
        writer: W,
    ) -> Result<(), PipelineError>
    where
        W: io::Write + Send + 'static,
        I: Iterator<Item = Value> + Send + 'static,
    {
        let writer: Box<dyn io::Write + Send> = Box::new(writer);
        let num_layout_threads = num_cpus::get().saturating_sub(1).max(4);
        let channel_buffer_size = num_layout_threads;

        info!("Starting pipeline with {} layout workers.", num_layout_threads);

        let (tx1, rx1) =
            async_channel::bounded::<Result<(usize, Arc<Value>), PipelineError>>(channel_buffer_size);
        let (tx2, rx2) =
            async_channel::bounded::<(usize, Result<LaidOutSequence, PipelineError>)>(channel_buffer_size);

        // --- STAGE 1: Producer ---
        let producer_handle = task::spawn(async move {
            info!("[PRODUCER] Starting sequence production from iterator.");
            for (i, item) in data_iterator.enumerate() {
                debug!("[PRODUCER] Sending item #{} to layout workers.", i);
                if tx1.send(Ok((i, Arc::new(item)))).await.is_err() {
                    warn!("[PRODUCER] Layout channel closed, stopping producer.");
                    break;
                }
            }
            info!("[PRODUCER] Finished sequence production.");
        });

        // --- STAGE 2: Layout Workers ---
        let layout_engine = LayoutEngine::new(Arc::clone(&self.font_manager));
        let mut layout_worker_handles = Vec::new();

        for worker_id in 0..num_layout_threads {
            let rx1_clone = rx1.clone();
            let tx2_clone = tx2.clone();
            let layout_engine_clone = layout_engine.clone();
            let template_clone = Arc::clone(&self.compiled_template);
            let debug_mode = self.debug;

            let worker_handle = task::spawn_blocking(move || {
                info!("[WORKER-{}] Started.", worker_id);
                while let Ok(result) = rx1_clone.recv_blocking() {
                    let (index, work_result) = match result {
                        Ok((index, context_arc)) => {
                            let data_source_string =
                                serde_json::to_string(&*context_arc)?;

                            let exec_config = ExecutionConfig {
                                format: DataSourceFormat::Json,
                                strict: debug_mode,
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
                                        debug_mode,
                                    )
                                });
                            (index, layout_result)
                        }
                        Err(e) => (0, Err(e)),
                    };

                    if tx2_clone.send_blocking((index, work_result)).is_err() {
                        warn!("[WORKER-{}] Consumer channel closed.", worker_id);
                        break;
                    }
                }
                info!("[WORKER-{}] Shutting down.", worker_id);
                Ok::<(), PipelineError>(())
            });
            layout_worker_handles.push(worker_handle);
        }
        drop(tx2);
        drop(rx1);

        // --- STAGE 3: Consumer / Renderer ---
        let pdf_backend = self.pdf_backend;
        let final_layout_engine = layout_engine.clone();
        let final_stylesheet = self.compiled_template.stylesheet();

        let consumer_handle = task::spawn_blocking(move || {
            info!("[CONSUMER] Started. Awaiting laid-out sequences.");
            let result: Result<(), PipelineError> = {
                let sequences = consume_and_reorder(rx2)?;

                // --- Analysis Pass (In-Memory) ---
                let mut pass1_result = Pass1Result::default();
                let mut global_page_offset = 0;
                for seq in &sequences {
                    pass1_result.total_pages += seq.pages.len();
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
                    global_page_offset += seq.pages.len();
                }

                // --- Generation & Assembly Pass ---
                let mut renderer: Box<dyn DocumentRenderer<Box<dyn io::Write + Send>>> =
                    match pdf_backend {
                        PdfBackend::PrintPdf | PdfBackend::PrintPdfParallel => {
                            warn!("The 'printpdf' backend is not fully supported by the new rendering pipeline yet.");
                            Box::new(PdfDocumentRenderer::new(final_layout_engine.clone(), final_stylesheet.clone())?)
                        }
                        PdfBackend::Lopdf | PdfBackend::LopdfParallel => {
                            Box::new(LopdfRenderer::new(final_layout_engine.clone(), final_stylesheet.clone())?)
                        }
                    };
                renderer.begin_document(writer)?;

                let default_master_name = final_stylesheet.default_page_master_name.as_ref().unwrap();
                let page_layout = final_stylesheet.page_masters.get(default_master_name).unwrap();
                let (page_width, page_height) = get_page_dimensions_pt(page_layout);

                // --- Content Rendering ---
                let mut page_content_ids = Vec::new();
                let mut fixup_elements_by_page: HashMap<usize, Vec<PositionedElement>> = HashMap::new();
                let mut global_page_idx = 0;

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
                                    element: LayoutElement::Text(TextElement {
                                        content: page_num_str,
                                        href: href.clone(),
                                        text_decoration: el.style.text_decoration.clone(),
                                    }),
                                    ..el.clone()
                                };
                                fixup_elements_by_page.entry(global_page_idx).or_default().push(text_el);
                            }
                        }

                        let content_id = renderer.render_page_content(page_elements.clone(), page_width, page_height)?;
                        page_content_ids.push(content_id);
                        global_page_idx += 1;
                    }
                }

                // --- Final Assembly ---
                let final_page_ids = if let Some(lopdf_renderer) = renderer.as_any_mut().downcast_mut::<LopdfRenderer<Box<dyn io::Write + Send>>>() {
                    // This IIFE captures the environment and executes the finalization logic.
                    // It's structured in phases to manage mutable borrows correctly.
                    let result: Result<Vec<lopdf::ObjectId>, PipelineError> = (|| {
                        // Phase 1: Use writer to create objects and get IDs for outlines and annotations.
                        // The borrow of `writer` is contained within this block.
                        let (final_page_ids, link_annots_by_page, outline_root_id) = {
                            let writer = lopdf_renderer.writer_mut()
                                .ok_or_else(|| PipelineError::Other("Renderer not initialized".to_string()))?;
                            let page_ids: Vec<_> = (0..pass1_result.total_pages).map(|_| writer.new_object_id()).collect();
                            let annots = lopdf_helpers::create_link_annotations(writer, &pass1_result, &sequences, &page_ids, page_height)?;
                            let outline = lopdf_helpers::build_outlines(writer, &pass1_result, &page_ids, page_height)?;
                            Ok::<_, PipelineError>((page_ids, annots, outline))
                        }?;

                        // Phase 2: Update renderer state now that `writer` is no longer borrowed.
                        if let Some(id) = outline_root_id {
                            lopdf_renderer.set_outline_root(id);
                        }

                        // Phase 3: Create fixup content streams. `writer` is borrowed again in a new, limited scope.
                        let mut fixup_content_streams = HashMap::new();
                        {
                            let writer = lopdf_renderer.writer_mut()
                                .ok_or_else(|| PipelineError::Other("Renderer not initialized".to_string()))?;
                            for (page_idx, elements) in fixup_elements_by_page {
                                let content = lopdf_helpers::render_elements_to_content(elements, &final_layout_engine, &final_stylesheet, page_width, page_height)?;
                                let content_id = writer.buffer_content_stream(content);
                                fixup_content_streams.insert(page_idx, content_id);
                            }
                        }

                        // Phase 4: Write the final page objects. This mutably borrows `lopdf_renderer`, which is now fine.
                        for i in 0..pass1_result.total_pages {
                            let mut contents = vec![page_content_ids[i]];
                            if let Some(fixup_id) = fixup_content_streams.get(&i) {
                                contents.push(*fixup_id);
                            }
                            let annots = link_annots_by_page.get(&i).cloned().unwrap_or_default();
                            let page_id = final_page_ids[i];
                            lopdf_renderer.write_page_object_at_id(page_id, contents, annots, page_width, page_height)?;
                        }

                        Ok(final_page_ids)
                    })();
                    result?
                } else {
                    vec![]
                };

                renderer.finish(final_page_ids).map_err(Into::into)
            };
            info!("[CONSUME] Finished.");
            result
        });

        producer_handle.await.unwrap();
        for handle in layout_worker_handles {
            handle.await.unwrap()?;
        }
        consumer_handle.await.unwrap()
    }

    pub fn generate_to_file<P: AsRef<Path>>(
        &self,
        data_iterator: impl Iterator<Item = Value> + Send + 'static,
        path: P,
    ) -> Result<(), PipelineError> {
        let output_path = path.as_ref();
        if let Some(parent_dir) = output_path.parent() {
            fs::create_dir_all(parent_dir)?;
        }
        let file = fs::File::create(output_path)?;
        let writer = io::BufWriter::new(file);

        let rt = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        rt.block_on(self.generate_to_writer_async(data_iterator, writer))
    }
}

/// Consumes all results from the worker channel and re-orders them into a vector.
fn consume_and_reorder(
    rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
) -> Result<Vec<LaidOutSequence>, PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_sequence_idx = 0;
    let mut all_sequences = Vec::new();

    while let Ok((index, result)) = rx.recv_blocking() {
        buffer.insert(index, result);
        while let Some(res) = buffer.remove(&next_sequence_idx) {
            let seq = res?;
            all_sequences.push(seq);
            next_sequence_idx += 1;
        }
    }
    Ok(all_sequences)
}

fn get_page_dimensions_pt(page_layout: &PageLayout) -> (f32, f32) {
    page_layout.size.dimensions_pt()
}