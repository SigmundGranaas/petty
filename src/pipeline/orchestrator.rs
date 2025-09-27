// FILE: /home/sigmund/RustroverProjects/petty/src/pipeline/orchestrator.rs
use super::config::{PdfBackend, Template};
use super::worker::{finish_layout_and_resource_loading, LaidOutSequence};
use crate::error::PipelineError;
use crate::parser::json::processor::JsonProcessor;
use crate::parser::xslt::executor::TemplateExecutor;
use crate::render::lopdf_renderer::{
    render_lopdf_page_to_bytes, LopdfDocumentRenderer, LopdfPageRenderTask,
};
use crate::render::pdf::{
    render_footer_to_ops, render_page_to_ops, PdfDocumentRenderer, RenderContext,
};
use crate::render::DocumentRenderer;
use async_channel;
use handlebars::{
    no_escape, Context, Handlebars, Helper, HelperResult, Output,
    RenderContext as HandlebarsRenderContext,
};
use log::{debug, info, warn};
use printpdf::{Layer, Op, PdfPage};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Builder;
use tokio::task::{self, JoinSet};
use crate::core::layout::{FontManager, LayoutEngine};
use crate::core::style::stylesheet::Stylesheet;

/// The main document generation pipeline.
/// It orchestrates parsing, layout, and rendering in a concurrent fashion.
pub struct DocumentPipeline {
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
    template: Template,
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
        stylesheet: Stylesheet,
        template: Template,
        pdf_backend: PdfBackend,
        font_manager: Arc<FontManager>,
        debug: bool,
    ) -> Self {
        let mut template_engine = Handlebars::new();
        template_engine.set_strict_mode(false);
        template_engine.register_helper("formatCurrency", Box::new(format_currency_helper));
        // Disable HTML escaping, as we are rendering to PDF, not HTML.
        template_engine.register_escape_fn(no_escape);

        DocumentPipeline {
            stylesheet,
            template_engine,
            template,
            pdf_backend,
            font_manager,
            debug,
        }
    }

    /// Generates a complete document and writes it to the provided `writer`.
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

        info!(
            "Starting pipeline with {} layout workers.",
            num_layout_threads
        );

        let (tx1, rx1) =
            async_channel::bounded::<Result<(usize, Arc<Value>), PipelineError>>(channel_buffer_size);
        let (tx2, rx2) = async_channel::bounded::<(usize, Result<LaidOutSequence, PipelineError>)>(
            channel_buffer_size,
        );

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
        let layout_engine = LayoutEngine::new(self.stylesheet.clone(), Arc::clone(&self.font_manager));
        let shared_template_engine = Arc::new(self.template_engine.clone());
        let mut layout_worker_handles = Vec::new();

        for worker_id in 0..num_layout_threads {
            let rx1_clone = rx1.clone();
            let tx2_clone = tx2.clone();
            let layout_engine_clone = layout_engine.clone();
            let template_engine_clone = Arc::clone(&shared_template_engine);
            let template_clone = self.template.clone();
            let debug_mode = self.debug;

            let worker_handle = task::spawn_blocking(move || {
                info!("[WORKER-{}] Started.", worker_id);
                while let Ok(result) = rx1_clone.recv_blocking() {
                    let (index, work_result) = match result {
                        Ok((index, context_arc)) => {
                            let parse_result = match &template_clone {
                                Template::Xslt(xslt_template) => {
                                    let mut executor = TemplateExecutor::new(
                                        &template_engine_clone,
                                        &xslt_template.compiled_stylesheet,
                                    );
                                    executor
                                        .build_tree(&context_arc)
                                        .map_err(PipelineError::from)
                                }
                                Template::Json(json_template) => {
                                    let processor = JsonProcessor::new(
                                        &json_template.template_content,
                                        &template_engine_clone,
                                    );
                                    processor
                                        .build_tree(&context_arc)
                                        .map_err(PipelineError::from)
                                }
                            };

                            let layout_result = parse_result.and_then(|ir_nodes| {
                                let base_path = match &template_clone {
                                    Template::Xslt(t) => &t.resource_base_path,
                                    Template::Json(t) => &t.resource_base_path,
                                };
                                finish_layout_and_resource_loading(
                                    worker_id,
                                    ir_nodes,
                                    context_arc.clone(),
                                    base_path,
                                    &layout_engine_clone,
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

        let consumer_handle = task::spawn_blocking(move || {
            info!("[CONSUMER] Started. Awaiting laid-out sequences.");
            let result = match pdf_backend {
                PdfBackend::PrintPdfParallel => consume_and_render_pages_printpdf_parallel(
                    rx2,
                    final_layout_engine,
                    consumer_template_engine,
                    writer,
                ),
                PdfBackend::LopdfParallel => consume_and_render_pages_lopdf_parallel(
                    rx2,
                    final_layout_engine,
                    consumer_template_engine,
                    writer,
                ),
                _ => {
                    let mut renderer: Box<dyn DocumentRenderer<Box<dyn io::Write + Send>>> =
                        match pdf_backend {
                            PdfBackend::PrintPdf => {
                                Box::new(PdfDocumentRenderer::new(final_layout_engine)?)
                            }
                            PdfBackend::Lopdf => {
                                Box::new(LopdfDocumentRenderer::new(final_layout_engine)?)
                            }
                            _ => unreachable!(),
                        };
                    renderer.begin_document(writer)?;
                    consume_and_render_pages_sequential(
                        rx2,
                        renderer.as_mut(),
                        &consumer_template_engine,
                    )?;
                    info!("[CONSUME] Finalizing document.");
                    renderer.finalize().map_err(Into::into)
                }
            };
            info!("[CONSUME] Finished.");
            result
        });

        // --- Wait for all stages to complete ---
        producer_handle.await.unwrap();
        for handle in layout_worker_handles {
            handle.await.unwrap();
        }
        consumer_handle.await.unwrap()
    }

    /// A convenience method that creates a file and runs the async pipeline.
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

// --- Consumer Implementations ---

/// Sequential Consumer: Renders pages one by one using a generic `DocumentRenderer`.
fn consume_and_render_pages_sequential<W: io::Write + Send>(
    rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
    renderer: &mut dyn DocumentRenderer<W>,
    template_engine: &Handlebars,
) -> Result<(), PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_sequence_to_render = 0;

    while let Ok((index, result)) = rx.recv_blocking() {
        buffer.insert(index, result);
        while let Some(res) = buffer.remove(&next_sequence_to_render) {
            let render_start_time = Instant::now();
            info!(
                "[CONSUMER] Rendering sequence #{}",
                next_sequence_to_render
            );
            let seq = res?;
            renderer.add_resources(&seq.resources)?;
            for (page_idx, page_elements) in seq.pages.into_iter().enumerate() {
                debug!(
                    "[CONSUMER] Rendering page {} of sequence #{}.",
                    page_idx + 1,
                    next_sequence_to_render
                );
                renderer.render_page(&seq.context, page_elements, template_engine)?;
            }
            info!(
                "[CONSUMER] Finished rendering sequence #{} in {:.2?}.",
                next_sequence_to_render,
                render_start_time.elapsed()
            );
            next_sequence_to_render += 1;
        }
    }
    Ok(())
}

type RenderChunkResult = Vec<(Vec<Op>, Option<Vec<Op>>)>;

/// Parallel Consumer for `PrintPdf` backend.
fn consume_and_render_pages_printpdf_parallel<W: io::Write + Send>(
    rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
    layout_engine: LayoutEngine,
    template_engine: Handlebars<'static>,
    writer: W,
) -> Result<(), PipelineError> {
    let mut renderer = PdfDocumentRenderer::new(layout_engine)?;
    renderer.begin_document(writer)?;

    let mut buffer = BTreeMap::new();
    let mut next_sequence_to_render = 0;
    let mut global_page_count = 0;

    let rt = tokio::runtime::Handle::current();
    let num_render_threads = num_cpus::get();
    let template_engine_arc = Arc::new(template_engine);

    while let Ok((index, result)) = rx.recv_blocking() {
        buffer.insert(index, result);

        while let Some(res) = buffer.remove(&next_sequence_to_render) {
            let seq = res?;
            if seq.pages.is_empty() {
                next_sequence_to_render += 1;
                continue;
            }

            // Sync: Add all unique images from this sequence to the document's resource cache
            renderer.add_resources(&seq.resources)?;

            // Parallel: Batch pages into chunks and render ops
            let mut render_tasks: JoinSet<Result<(usize, RenderChunkResult), PipelineError>> =
                JoinSet::new();
            let chunk_size = (seq.pages.len() + num_render_threads - 1) / num_render_threads;
            let layout_engine_arc = Arc::new(renderer.layout_engine.clone());
            let image_xobjects_arc = Arc::new(renderer.image_xobjects.clone());
            let fonts_arc = Arc::new(renderer.fonts.clone());
            let default_font_arc = Arc::new(renderer.default_font.clone());
            let ss_clone = Arc::new(renderer.stylesheet.clone());
            let (_, page_height_pt) =
                PdfDocumentRenderer::<W>::get_page_dimensions_pt(&ss_clone.page);

            for (chunk_idx, page_chunk) in seq.pages.chunks(chunk_size).enumerate() {
                let task_data = (
                    page_chunk.to_vec(),
                    global_page_count + (chunk_idx * chunk_size) + 1,
                    Arc::clone(&seq.context),
                    Arc::clone(&layout_engine_arc),
                    Arc::clone(&template_engine_arc),
                    Arc::clone(&image_xobjects_arc),
                    Arc::clone(&fonts_arc),
                    Arc::clone(&default_font_arc),
                    Arc::clone(&ss_clone),
                );
                render_tasks.spawn_blocking_on(
                    move || {
                        let (
                            page_chunk_owned,
                            start_page_num,
                            context_clone,
                            le_clone,
                            te_clone,
                            ix_clone,
                            f_clone,
                            df_clone,
                            ss_clone_inner,
                        ) = task_data;
                        let mut chunk_results = Vec::with_capacity(page_chunk_owned.len());
                        for (i, page_elements) in page_chunk_owned.into_iter().enumerate() {
                            let ctx = RenderContext {
                                image_xobjects: &ix_clone,
                                fonts: &f_clone,
                                default_font: &df_clone,
                                page_height_pt,
                            };
                            let content_ops = render_page_to_ops(ctx, page_elements)?;
                            let footer_ops = render_footer_to_ops::<W>(
                                &le_clone,
                                &f_clone,
                                &df_clone,
                                &context_clone,
                                &ss_clone_inner.page,
                                start_page_num + i,
                                &te_clone,
                            )?;
                            chunk_results.push((content_ops, footer_ops));
                        }
                        Ok((chunk_idx, chunk_results))
                    },
                    &rt,
                );
            }

            // Collect and order rendered chunks
            let mut rendered_chunks = BTreeMap::new();
            while let Some(join_result) = rt.block_on(render_tasks.join_next()) {
                let (chunk_idx, chunk_data) = join_result.unwrap()?;
                rendered_chunks.insert(chunk_idx, chunk_data);
            }

            // Sync: Add rendered pages to the document
            let (page_width_mm, page_height_mm) =
                PdfDocumentRenderer::<W>::get_page_dimensions_mm(&renderer.stylesheet.page);
            for chunk_data in rendered_chunks.into_values() {
                for (content_ops, footer_ops) in chunk_data {
                    global_page_count += 1;
                    let mut final_ops = Vec::new();
                    let layer_name = format!("Page {} Layer 1", global_page_count);
                    let layer_id = renderer.document.add_layer(&Layer::new(&layer_name));
                    final_ops.push(Op::BeginLayer { layer_id });
                    final_ops.extend(content_ops);
                    if let Some(ops) = footer_ops {
                        final_ops.extend(ops);
                    }
                    renderer.document.pages.push(PdfPage::new(
                        page_width_mm,
                        page_height_mm,
                        final_ops,
                    ));
                }
            }
            next_sequence_to_render += 1;
        }
    }
    Box::new(renderer).finalize().map_err(Into::into)
}

/// Parallel Consumer for `Lopdf` streaming backend.
fn consume_and_render_pages_lopdf_parallel<W: io::Write + Send>(
    rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
    layout_engine: LayoutEngine,
    template_engine: Handlebars<'static>,
    writer: W,
) -> Result<(), PipelineError> {
    let mut renderer = LopdfDocumentRenderer::new(layout_engine)?;
    renderer.begin_document(writer)?;

    let mut buffer = BTreeMap::new();
    let mut next_sequence_to_render = 0;
    let mut global_page_count = 0;

    let rt = tokio::runtime::Handle::current();
    let num_render_threads = num_cpus::get();
    let template_engine_arc = Arc::new(template_engine);

    while let Ok((index, result)) = rx.recv_blocking() {
        buffer.insert(index, result);

        while let Some(res) = buffer.remove(&next_sequence_to_render) {
            let seq = res?;
            if seq.pages.is_empty() {
                next_sequence_to_render += 1;
                continue;
            }

            // NOTE: Lopdf backend doesn't support images yet, so seq.resources is unused here.
            // When it does, resources will need to be added to the document before this step.

            let writer_mut = renderer.writer.as_mut().unwrap();
            let page_tasks: Vec<LopdfPageRenderTask> = seq
                .pages
                .into_iter()
                .map(|elems| LopdfPageRenderTask {
                    page_object_id: writer_mut.new_object_id(),
                    content_object_id: writer_mut.new_object_id(),
                    elements: elems,
                    context: Arc::clone(&seq.context),
                })
                .collect();
            writer_mut.add_page_ids(page_tasks.iter().map(|t| t.page_object_id));

            let mut render_tasks: JoinSet<Result<(usize, Vec<u8>), PipelineError>> = JoinSet::new();
            let chunk_size = (page_tasks.len() + num_render_threads - 1) / num_render_threads;
            let layout_engine_arc = Arc::new(renderer.layout_engine.clone());
            let stylesheet_arc = Arc::clone(&renderer.stylesheet);
            let resources_id = writer_mut.resources_id;
            let parent_pages_id = writer_mut.pages_id;

            for (chunk_idx, task_chunk) in page_tasks.chunks(chunk_size).enumerate() {
                let task_data = (
                    task_chunk.to_vec(),
                    global_page_count + (chunk_idx * chunk_size) + 1,
                    Arc::clone(&layout_engine_arc),
                    Arc::clone(&template_engine_arc),
                    Arc::clone(&stylesheet_arc),
                );

                render_tasks.spawn_blocking_on(
                    move || {
                        let (task_chunk_owned, start_page_num, le_clone, te_clone, ss_clone) =
                            task_data;
                        let mut chunk_bytes = Vec::new();
                        for (i, task) in task_chunk_owned.into_iter().enumerate() {
                            let bytes = render_lopdf_page_to_bytes(
                                task,
                                &ss_clone.page,
                                start_page_num + i,
                                resources_id,
                                parent_pages_id,
                                &le_clone,
                                &te_clone,
                            )?;
                            chunk_bytes.extend(bytes);
                        }
                        Ok((chunk_idx, chunk_bytes))
                    },
                    &rt,
                );
            }

            let mut rendered_chunks = BTreeMap::new();
            while let Some(join_result) = rt.block_on(render_tasks.join_next()) {
                let (chunk_idx, bytes) = join_result.unwrap()?;
                rendered_chunks.insert(chunk_idx, bytes);
            }

            for bytes in rendered_chunks.into_values() {
                renderer
                    .writer
                    .as_mut()
                    .unwrap()
                    .write_pre_rendered_objects(bytes)?;
            }
            global_page_count += page_tasks.len();
            next_sequence_to_render += 1;
        }
    }
    Box::new(renderer).finalize().map_err(Into::into)
}