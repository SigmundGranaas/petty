use super::config::PdfBackend;
use super::worker::{finish_layout_and_resource_loading, LaidOutSequence};
use crate::core::layout::{FontManager, LayoutEngine};
use crate::error::PipelineError;
use crate::parser::processor::{CompiledTemplate, DataSourceFormat, ExecutionConfig};
use crate::render::lopdf_renderer::LopdfDocumentRenderer;
use crate::render::pdf::PdfDocumentRenderer;
use crate::render::renderer::ResolvedAnchor;
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
use std::time::Instant;
use tokio::runtime::Builder;
use tokio::task;

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
                                serde_json::to_string(&*context_arc).map_err(PipelineError::from).unwrap();

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
                                        template_clone.stylesheet(),
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
            });
            layout_worker_handles.push(worker_handle);
        }
        drop(tx2);
        drop(rx1);

        // --- STAGE 3: Consumer / Renderer ---
        let consumer_template_engine = self.template_engine.clone();
        let pdf_backend = self.pdf_backend;
        let final_layout_engine = layout_engine.clone();
        let final_stylesheet = self.compiled_template.stylesheet().clone();

        let consumer_handle = task::spawn_blocking(move || {
            info!("[CONSUMER] Started. Awaiting laid-out sequences.");
            let result = {
                let mut renderer: Box<dyn DocumentRenderer<Box<dyn io::Write + Send>>> =
                    match pdf_backend {
                        PdfBackend::PrintPdf | PdfBackend::PrintPdfParallel => Box::new(PdfDocumentRenderer::new(
                            final_layout_engine,
                            final_stylesheet,
                        )?),
                        PdfBackend::Lopdf | PdfBackend::LopdfParallel => Box::new(LopdfDocumentRenderer::new(
                            final_layout_engine,
                            final_stylesheet,
                        )?),
                    };
                renderer.begin_document(writer)?;
                let (sequences, anchors) = consume_and_render_pages_sequential(
                    rx2,
                    renderer.as_mut(),
                    &consumer_template_engine,
                )?;
                info!("[CONSUME] Finalizing document.");
                renderer.finalize(&anchors, &sequences)?;
                renderer.finish().map_err(Into::into)
            };
            info!("[CONSUME] Finished.");
            result
        });

        producer_handle.await.unwrap();
        for handle in layout_worker_handles {
            handle.await.unwrap();
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

fn consume_and_render_pages_sequential<W: io::Write + Send>(
    rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
    renderer: &mut dyn DocumentRenderer<W>,
    template_engine: &Handlebars,
) -> Result<(Vec<LaidOutSequence>, HashMap<String, ResolvedAnchor>), PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_sequence_to_render = 0;
    let mut global_page_offset = 0;
    let mut all_sequences = Vec::new();
    let mut resolved_anchors = HashMap::new();

    while let Ok((index, result)) = rx.recv_blocking() {
        buffer.insert(index, result);
        while let Some(res) = buffer.remove(&next_sequence_to_render) {
            let render_start_time = Instant::now();
            info!("[CONSUMER] Rendering sequence #{}", next_sequence_to_render);
            let mut seq = res?;
            renderer.add_resources(&seq.resources)?;

            for (local_page_idx, page_elements) in seq.pages.iter_mut().enumerate() {
                for (name, anchor) in &seq.defined_anchors {
                    if anchor.local_page_index == local_page_idx {
                        resolved_anchors.insert( name.clone(), ResolvedAnchor {
                            global_page_index: global_page_offset + local_page_idx + 1,
                            y_pos: anchor.y_pos,
                        },
                        );
                    }
                }
                debug!("[CONSUMER] Rendering page {} of sequence #{}.", local_page_idx + 1, next_sequence_to_render);
                let elements_to_render = std::mem::take(page_elements);
                renderer.render_page(&seq.context, elements_to_render, template_engine)?;
            }

            global_page_offset += seq.pages.len();
            all_sequences.push(seq);
            info!("[CONSUMER] Finished rendering sequence #{} in {:.2?}.", next_sequence_to_render, render_start_time.elapsed());
            next_sequence_to_render += 1;
        }
    }
    Ok((all_sequences, resolved_anchors))
}