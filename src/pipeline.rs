// src/pipeline.rs

use crate::error::PipelineError;
use crate::idf::{IRNode, LayoutUnit};
use crate::layout::LayoutEngine;
use crate::layout::PositionedElement;
use crate::parser::xslt;
use crate::parser::xslt::builder::{PreparsedTemplate, TreeBuilder};
use crate::render::lopdf::LopdfDocumentRenderer;
use crate::render::pdf::PdfDocumentRenderer;
use crate::render::DocumentRenderer;
use crate::stylesheet::Stylesheet;
use crate::xpath;
use async_channel;
use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext};
use log::{debug, info, warn};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Builder;
use tokio::task;

/// An enum to select the desired PDF rendering backend.
#[derive(Debug, Clone, Copy, Default)]
pub enum PdfBackend {
    PrintPdf,
    #[default]
    Lopdf,
}

pub struct DocumentPipeline {
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
    template_language: TemplateLanguage,
    pdf_backend: PdfBackend,
}

#[derive(Clone)]
pub enum TemplateLanguage {
    Json,
    Xslt {
        xslt_content: String,
        preparsed_template: PreparsedTemplate,
    },
}

struct LaidOutSequence {
    context: Arc<Value>,
    pages: Vec<Vec<PositionedElement>>,
}

fn format_currency_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
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
        template_language: TemplateLanguage,
        pdf_backend: PdfBackend,
    ) -> Self {
        let mut template_engine = Handlebars::new();
        template_engine.set_strict_mode(true);
        template_engine.register_helper("formatCurrency", Box::new(format_currency_helper));

        DocumentPipeline {
            stylesheet,
            template_engine,
            template_language,
            pdf_backend,
        }
    }

    pub async fn generate_to_writer_async<W: io::Write + Send + 'static>(
        &self,
        data: &Value,
        writer: W,
    ) -> Result<(), PipelineError> {
        let writer: Box<dyn io::Write + Send> = Box::new(writer);

        // Calculate the number of workers first.
        let num_layout_threads = num_cpus::get().saturating_sub(1).max(4);
        // Set the buffer size to match the number of workers. This creates backpressure,
        // preventing the layout stage from getting too far ahead of the rendering bottleneck.
        let channel_buffer_size = num_layout_threads;

        info!("Using channel buffer size: {}", channel_buffer_size);

        let (tx1, rx1) =
            async_channel::bounded::<Result<(usize, Arc<Value>), PipelineError>>(channel_buffer_size);
        let (tx2, rx2) =
            async_channel::bounded::<(usize, Result<LaidOutSequence, PipelineError>)>(channel_buffer_size);

        let data = data.clone();
        let producer_stylesheet = self.stylesheet.clone();
        let template_language = self.template_language.clone();

        let producer_handle = task::spawn(async move {
            info!("[PRODUCE] Starting sequence production.");
            match get_sequence_data(&data, &producer_stylesheet, &template_language) {
                Ok(data_items) => {
                    info!("[PRODUCE] Found {} sequences to process.", data_items.len());
                    for (i, item) in data_items.into_iter().enumerate() {
                        debug!("[PRODUCE] Sending sequence #{} to layout workers.", i);
                        if tx1.send(Ok((i, Arc::new(item)))).await.is_err() {
                            warn!("[PRODUCE] Layout channel closed, stopping producer.");
                            break;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx1.send(Err(e)).await;
                }
            };
            info!("[PRODUCE] Finished sequence production.");
        });

        info!(
            "[MANAGER] Spawning {} layout worker threads.",
            num_layout_threads
        );

        let layout_engine = LayoutEngine::new(self.stylesheet.clone());
        let shared_template_engine = Arc::new(self.template_engine.clone());
        let mut layout_worker_handles = Vec::new();

        for worker_id in 0..num_layout_threads {
            let rx1_clone = rx1.clone();
            let tx2_clone = tx2.clone();
            let layout_engine_clone = layout_engine.clone();
            let template_engine_clone = Arc::clone(&shared_template_engine);
            let template_language_clone = self.template_language.clone();

            let worker_handle = task::spawn_blocking(move || {
                info!("[WORKER-{}] Started.", worker_id);
                // Create the TreeBuilder ONCE per worker, before the loop.
                let mut builder = TreeBuilder::new(&template_engine_clone);

                while let Ok(result) = rx1_clone.recv_blocking() {
                    let (index, work_result) = match result {
                        Ok((index, context_arc)) => {
                            debug!("[WORKER-{}] Received sequence #{}.", worker_id, index);
                            // Pass the mutable builder into the function.
                            let res = do_parse_and_layout(
                                worker_id,
                                &mut builder,
                                context_arc,
                                &template_language_clone,
                                &layout_engine_clone,
                            );
                            (index, res)
                        }
                        Err(e) => (0, Err(e)),
                    };

                    if tx2_clone.send_blocking((index, work_result)).is_err() {
                        warn!(
                            "[WORKER-{}] Consumer channel closed, stopping worker.",
                            worker_id
                        );
                        break;
                    }
                }
                info!("[WORKER-{}] Shutting down.", worker_id);
            });
            layout_worker_handles.push(worker_handle);
        }
        drop(tx2);
        drop(rx1);

        let consumer_template_engine = self.template_engine.clone();
        let pdf_backend = self.pdf_backend;
        let final_layout_engine = layout_engine.clone();

        let consumer_handle = task::spawn_blocking(move || {
            info!("[CONSUME] Started. Awaiting laid-out sequences.");
            let res = || -> Result<(), PipelineError> {
                let mut renderer: Box<dyn DocumentRenderer> = match pdf_backend {
                    PdfBackend::PrintPdf => {
                        Box::new(PdfDocumentRenderer::new(final_layout_engine)?)
                    }
                    PdfBackend::Lopdf => Box::new(LopdfDocumentRenderer::new(final_layout_engine)?),
                };

                renderer.begin_document()?;
                consume_and_render_pages(rx2, renderer.as_mut(), &consumer_template_engine)?;

                let finalize_start = Instant::now();
                info!("[CONSUME] Finalizing document.");
                renderer.finalize(writer)?;
                let finalize_duration = finalize_start.elapsed();
                info!("[CONSUME] Document finalized successfully in {:.2?}.", finalize_duration);
                Ok(())
            }();
            res
        });

        producer_handle
            .await
            .map_err(|e| PipelineError::TemplateParseError(format!("Producer task panicked: {}", e)))?;

        for handle in layout_worker_handles {
            handle.await.map_err(|e| {
                PipelineError::TemplateParseError(format!("Layout worker task panicked: {}", e))
            })?;
        }

        consumer_handle.await.unwrap()
    }

    pub fn generate_to_file<P: AsRef<Path>>(
        &self,
        data: &Value,
        path: P,
    ) -> Result<(), PipelineError> {
        let file = std::fs::File::create(path)?;
        let rt = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");
        rt.block_on(self.generate_to_writer_async(data, file))
    }
}

fn get_sequence_data(
    data: &Value,
    stylesheet: &Stylesheet,
    lang: &TemplateLanguage,
) -> Result<Vec<Value>, PipelineError> {
    match lang {
        TemplateLanguage::Json => {
            let seq = stylesheet.page_sequences.values().next().ok_or_else(|| {
                PipelineError::StylesheetError("No page_sequences in stylesheet".to_string())
            })?;
            let data_items = xpath::select(data, &seq.data_source);
            if let Some(first_item) = data_items.get(0) {
                if let Some(arr) = first_item.as_array() {
                    return Ok(arr.clone());
                }
            }
            Ok(data_items.into_iter().cloned().collect())
        }
        TemplateLanguage::Xslt { .. } => {
            let path = xslt::get_sequence_path(&stylesheet.page_sequences)?;
            let results = xpath::select(data, &path);
            let items: Vec<Value> = if path == "." || path == "/" {
                vec![data.clone()]
            } else if let Some(first_val) = results.get(0) {
                if let Some(arr) = first_val.as_array() {
                    arr.clone()
                } else {
                    results.into_iter().cloned().collect()
                }
            } else {
                Vec::new()
            };
            Ok(items)
        }
    }
}

// Function now accepts a mutable reference to a TreeBuilder.
fn do_parse_and_layout<'h>(
    worker_id: usize,
    builder: &mut TreeBuilder<'h>,
    context_arc: Arc<Value>,
    lang: &TemplateLanguage,
    layout_engine: &LayoutEngine,
) -> Result<LaidOutSequence, PipelineError> {
    let total_start = Instant::now();
    info!("[WORKER-{}] Starting processing sequence.", worker_id);

    let tree = match lang {
        TemplateLanguage::Xslt {
            preparsed_template, ..
        } => {
            let parse_start = Instant::now();
            debug!(
                "[WORKER-{}] Starting XSLT build from pre-parsed template.",
                worker_id
            );
            // Use the passed-in builder instead of creating a new one.
            let children = builder.build_tree_from_preparsed(preparsed_template, &context_arc)?;
            let parse_duration = parse_start.elapsed();
            debug!(
                "[WORKER-{}] Finished XSLT build in {:.2?}.",
                worker_id, parse_duration
            );
            IRNode::Root(children)
        }
        TemplateLanguage::Json => {
            return Err(PipelineError::StylesheetError(
                "Parallel processing for JSON templates not fully implemented in this refactor."
                    .to_string(),
            ));
        }
    };

    let layout_unit = LayoutUnit {
        tree,
        context: Arc::clone(&context_arc),
    };

    let layout_start = Instant::now();
    debug!("[WORKER-{}] Paginating sequence tree.", worker_id);
    let pages: Vec<Vec<PositionedElement>> =
        layout_engine.paginate_tree(layout_unit)?.collect();
    let layout_duration = layout_start.elapsed();
    debug!(
        "[WORKER-{}] Finished paginating sequence ({} pages) in {:.2?}.",
        worker_id,
        pages.len(),
        layout_duration
    );

    let total_duration = total_start.elapsed();
    info!(
        "[WORKER-{}] Finished processing sequence in {:.2?}.",
        worker_id, total_duration
    );

    Ok(LaidOutSequence {
        context: context_arc,
        pages,
    })
}

fn consume_and_render_pages<R: DocumentRenderer + ?Sized>(
    rx: async_channel::Receiver<(usize, Result<LaidOutSequence, PipelineError>)>,
    renderer: &mut R,
    template_engine: &Handlebars,
) -> Result<(), PipelineError> {
    let mut buffer = BTreeMap::new();
    let mut next_sequence_to_render = 0;

    while let Ok((index, result)) = rx.recv_blocking() {
        debug!("[CONSUME] Received laid-out sequence #{}", index);
        buffer.insert(index, result);
        while let Some(res) = buffer.remove(&next_sequence_to_render) {
            let render_start_time = Instant::now();
            info!("[CONSUME] Rendering sequence #{}", next_sequence_to_render);
            let seq = res?;
            for (page_idx, page_elements) in seq.pages.into_iter().enumerate() {
                debug!(
                    "[CONSUME] Rendering page {} of sequence #{}.",
                    page_idx + 1,
                    next_sequence_to_render
                );
                renderer.render_page(&seq.context, page_elements, template_engine)?;
            }
            let render_duration = render_start_time.elapsed();
            info!(
                "[CONSUME] Finished rendering sequence #{} in {:.2?}.",
                next_sequence_to_render, render_duration
            );
            next_sequence_to_render += 1;
        }
    }
    info!("[CONSUME] All sequences received and rendered.");
    Ok(())
}

#[derive(Default)]
pub struct PipelineBuilder {
    stylesheet: Option<Stylesheet>,
    template_language: Option<TemplateLanguage>,
    pdf_backend: PdfBackend,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_stylesheet_json(mut self, json: &str) -> Result<Self, PipelineError> {
        let stylesheet = Stylesheet::from_json(json)?;
        self.stylesheet = Some(stylesheet);
        self.template_language = Some(TemplateLanguage::Json);
        Ok(self)
    }

    pub fn with_stylesheet_file<P: AsRef<Path>>(self, path: P) -> Result<Self, PipelineError> {
        let json_str = fs::read_to_string(path)?;
        self.with_stylesheet_json(&json_str)
    }

    pub fn with_xslt_template_file<P: AsRef<Path>>(
        mut self,
        path: P,
    ) -> Result<Self, PipelineError> {
        let xslt_content = fs::read_to_string(path)?;
        let stylesheet = Stylesheet::from_xslt(&xslt_content)?;

        // Pre-parse the template right here, once.
        let sequence_template_str = xslt::extract_sequence_template(&xslt_content)?;
        let handlebars = Handlebars::new(); // Dummy engine for preparsing
        let builder = TreeBuilder::new(&handlebars);
        let preparsed_template = builder.preparse_from_str(&sequence_template_str)?;

        self.stylesheet = Some(stylesheet);
        self.template_language = Some(TemplateLanguage::Xslt {
            xslt_content,
            preparsed_template,
        });
        Ok(self)
    }

    pub fn with_pdf_backend(mut self, backend: PdfBackend) -> Self {
        self.pdf_backend = backend;
        self
    }

    pub fn build(self) -> Result<DocumentPipeline, PipelineError> {
        let stylesheet = self.stylesheet.ok_or_else(|| {
            PipelineError::StylesheetError("No stylesheet or template provided".to_string())
        })?;
        let language = self.template_language.ok_or_else(|| {
            PipelineError::StylesheetError("Template language could not be determined".to_string())
        })?;
        let generator = DocumentPipeline::new(stylesheet, language, self.pdf_backend);
        Ok(generator)
    }
}