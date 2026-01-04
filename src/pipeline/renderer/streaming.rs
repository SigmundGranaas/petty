use crate::MapRenderError;
use crate::pipeline::api::PreparedDataSources;
use crate::pipeline::concurrency::{
    DynamicWorkerPool, producer_task, run_in_order_streaming_consumer, spawn_workers,
};
use crate::pipeline::config::PdfBackend;
use crate::pipeline::context::PipelineContext;
use crate::pipeline::renderer::RenderingStrategy;
use log::{debug, info, warn};
use petty_core::error::PipelineError;
use petty_render_core::DocumentRenderer;
use petty_render_lopdf::LopdfRenderer;
use std::io::{Seek, Write};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;

// Need to import LayoutEngine for the consumer stage.
use petty_core::layout::LayoutEngine;

/// A rendering strategy that streams the document directly to the output.
///
/// This renderer processes documents in a streaming fashion, rendering pages
/// as they are laid out without buffering the entire document in memory.
///
/// # Worker Configuration
///
/// The number of layout worker threads can be configured in order of priority:
/// 1. Explicit `worker_count` in configuration
/// 2. `PETTY_WORKER_COUNT` environment variable
/// 3. Auto-detect: `physical_cores / 2 + 1` (benchmarks show ~half physical cores is optimal)
///
/// # Example
///
/// ```ignore
/// use petty::pipeline::renderer::streaming::SinglePassStreamingRenderer;
/// use petty::PdfBackend;
///
/// // Simple construction with defaults
/// let renderer = SinglePassStreamingRenderer::new(PdfBackend::Lopdf);
///
/// // Advanced configuration
/// let renderer = SinglePassStreamingRenderer::builder()
///     .backend(PdfBackend::Lopdf)
///     .worker_count(8)
///     .build();
/// ```
/// Default render buffer size for pipelining.
/// Benchmarks show smaller buffers (16) outperform larger ones.
const DEFAULT_RENDER_BUFFER_SIZE: usize = 16;

#[derive(Clone)]
pub struct SinglePassStreamingRenderer {
    pdf_backend: PdfBackend,
    /// Optional override for worker count (None = auto-detect)
    worker_count: Option<usize>,
    /// Buffer size for async PDF writing pipeline
    render_buffer_size: usize,
}

impl SinglePassStreamingRenderer {
    /// Create a new renderer with the specified PDF backend.
    ///
    /// Uses default settings:
    /// - Worker count: auto-detected (~half physical cores + 1)
    /// - Render buffer: 16 pages
    ///
    /// For more control, use [`with_config`](Self::with_config).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let renderer = SinglePassStreamingRenderer::new(PdfBackend::Lopdf);
    /// ```
    #[allow(dead_code)] // Public API - may be used by external consumers
    pub fn new(pdf_backend: PdfBackend) -> Self {
        Self::with_config(pdf_backend, None, DEFAULT_RENDER_BUFFER_SIZE)
    }

    /// Create a new renderer with explicit configuration.
    ///
    /// # Arguments
    ///
    /// * `pdf_backend` - The PDF backend to use for rendering
    /// * `worker_count` - Optional explicit worker count (None = auto-detect)
    /// * `render_buffer_size` - Number of rendered pages to buffer before writing
    ///
    /// The render buffer enables async PDF writing by allowing rendering to
    /// continue while I/O is in progress. Higher values trade memory for throughput.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let renderer = SinglePassStreamingRenderer::with_config(
    ///     PdfBackend::Lopdf,
    ///     Some(4),   // Use 4 workers
    ///     128,       // Buffer 128 pages
    /// );
    /// ```
    pub fn with_config(
        pdf_backend: PdfBackend,
        worker_count: Option<usize>,
        render_buffer_size: usize,
    ) -> Self {
        Self {
            pdf_backend,
            worker_count,
            render_buffer_size: render_buffer_size.max(1),
        }
    }

    /// Determine the number of layout threads to use based on configuration.
    ///
    /// Priority: explicit config > env var > auto-detect
    ///
    /// Benchmarks show optimal throughput at approximately half the physical core count + 1.
    /// More workers cause contention; fewer leave cores idle.
    fn get_worker_count(&self) -> usize {
        self.worker_count.unwrap_or_else(|| {
            std::env::var("PETTY_WORKER_COUNT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| {
                    // Use ~half of physical cores + 1 for optimal throughput
                    // Leaves cores available for consumer, producer, and I/O
                    let physical = num_cpus::get_physical();
                    (physical / 2 + 1).max(2)
                })
        })
    }
}

impl RenderingStrategy for SinglePassStreamingRenderer {
    fn render<W>(
        &self,
        context: &PipelineContext,
        sources: PreparedDataSources,
        writer: W,
    ) -> Result<W, PipelineError>
    where
        W: Write + Seek + Send + 'static,
    {
        if sources.document.is_some() {
            warn!(
                "SinglePassStreamingRenderer received Document metadata but cannot use it. The metadata will be ignored."
            );
        }
        if sources.body_artifact.is_some() {
            warn!(
                "SinglePassStreamingRenderer received a pre-rendered body artifact but cannot use it. The artifact will be ignored."
            );
        }

        if !matches!(
            self.pdf_backend,
            PdfBackend::Lopdf | PdfBackend::LopdfParallel
        ) {
            return Err(PipelineError::Config(
                "SinglePassStreamingRenderer only supports the 'Lopdf' or 'LopdfParallel' backend."
                    .into(),
            ));
        }

        // Use dynamic worker count configuration (no artificial cap)
        let num_layout_threads = self.get_worker_count();

        // Warn if worker count exceeds physical cores (hyperthreading overhead)
        let physical_cores = num_cpus::get_physical();
        if num_layout_threads > physical_cores {
            warn!(
                "Worker count ({}) exceeds physical cores ({}). \
                 Performance may degrade due to hyperthreading overhead. \
                 Consider using {} workers for optimal throughput.",
                num_layout_threads,
                physical_cores,
                physical_cores.saturating_sub(1).max(2)
            );
        }

        // Work channel: 2x worker count to reduce blocking contention
        let work_channel_size = num_layout_threads * 2;
        // Results channel: use configured render buffer size for buffering
        // This allows workers to produce ahead of consumer processing
        let results_channel_size = self.render_buffer_size;

        // Get max_in_flight_buffer from config, default to 2 if no adaptive facade
        let buffer_headroom = context
            .adaptive
            .as_ref()
            .map(|f| f.max_in_flight_buffer())
            .unwrap_or(2);
        let max_in_flight = num_layout_threads + buffer_headroom;
        let semaphore = Arc::new(Semaphore::new(max_in_flight));

        info!(
            "Starting Single-Pass Streaming pipeline with {} layout workers (Max in-flight: {}, Buffer: {}).",
            num_layout_threads, max_in_flight, results_channel_size
        );

        let (tx1, rx1) = async_channel::bounded(work_channel_size);
        let (tx2, rx2) = async_channel::bounded(results_channel_size);

        let producer = task::spawn(producer_task(sources.data_iterator, tx1, semaphore.clone()));
        let workers = spawn_workers(num_layout_threads, context, rx1.clone(), tx2.clone());

        // Create dynamic worker pool for adaptive scaling (when worker manager is available)
        let mut worker_pool = context.worker_manager().map(|wm| {
            DynamicWorkerPool::new(
                Arc::new(context.clone()),
                rx1.clone(),
                wm,
                num_layout_threads,
            )
        });

        // Determine if we need adaptive scaling (dynamic worker spawning)
        let has_adaptive_scaling = worker_pool.is_some();

        // CRITICAL: Drop original tx2 BEFORE consumer starts!
        // Workers already have clones. If we hold tx2, the channel won't close
        // when workers finish, causing consumer to block forever.
        //
        // For adaptive mode: clone tx2 FIRST, then drop original
        let result_sender = if has_adaptive_scaling {
            let sender_for_consumer = tx2.clone();
            drop(tx2); // Drop original - workers + consumer clone remain
            debug!("[STREAMING] Dropped original tx2 (adaptive mode), keeping consumer clone");
            Some(sender_for_consumer)
        } else {
            drop(tx2); // Drop original - only worker clones remain
            debug!("[STREAMING] Dropped original tx2 (non-adaptive mode)");
            None
        };

        // --- Consumer Stage ---
        info!("[CONSUMER] Started in-order streaming consumer. Awaiting laid-out sequences.");
        let final_layout_engine = LayoutEngine::new(&context.font_library, context.cache_config);
        let final_stylesheet = context.compiled_template.stylesheet();

        // Pass Arc<Stylesheet> correctly
        let mut renderer =
            LopdfRenderer::new(final_layout_engine, final_stylesheet.clone()).map_render_err()?;
        renderer.begin_document(writer).map_render_err()?;

        let (page_width, page_height) = renderer
            .stylesheet
            .get_default_page_layout()
            .size
            .dimensions_pt();

        let (all_page_ids, _) = run_in_order_streaming_consumer(
            rx2,
            &mut renderer,
            page_width,
            page_height,
            false,
            semaphore,
            context.adaptive_controller(),
            worker_pool.as_mut(),
            result_sender,
        )?;

        let writer = Box::new(renderer).finish(all_page_ids).map_render_err()?;

        producer.abort();
        for worker in workers {
            worker.abort();
        }

        // Abort dynamically spawned workers
        if let Some(pool) = worker_pool {
            let handles = pool.abort_all();
            for handle in handles {
                drop(handle);
            }
        }

        info!("[CONSUMER] Finished streaming.");
        Ok(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::adapters::TemplateParserAdapter;
    use crate::pipeline::provider::DataSourceProvider;
    use crate::pipeline::provider::passthrough::PassThroughProvider;
    use petty_core::layout::fonts::SharedFontLibrary;
    use petty_core::parser::processor::TemplateParser;
    use petty_json_template::JsonParser;
    use serde_json::json;
    use std::io::{Cursor, Read, SeekFrom};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_streaming_pipeline_integration() {
        let template_json = json!({
            "_stylesheet": {
                "defaultPageMaster": "default",
                "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
                "styles": { "default": { "font-family": "Helvetica" } }
            },
            "_template": {
                "type": "Paragraph",
                "children": [ { "type": "Text", "content": "Hello {{name}}" } ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();
        let parser = TemplateParserAdapter::new(JsonParser);
        let features = parser.parse(&template_str, PathBuf::new()).unwrap();
        let library = SharedFontLibrary::new();
        library.load_fallback_font();

        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(features.role_templates),
            font_library: Arc::new(library),
            resource_provider: Arc::new(petty_resource::InMemoryResourceProvider::new()),
            cache_config: Default::default(),
            adaptive: None,
        };

        let provider = PassThroughProvider;
        let renderer = SinglePassStreamingRenderer::with_config(PdfBackend::Lopdf, None, 16);

        let data = vec![json!({"name": "World"})];
        let iterator = data.into_iter();

        let prepared_sources = provider.provide(&context, iterator).unwrap();
        let writer = Cursor::new(Vec::new());

        let mut final_writer = tokio::task::spawn_blocking(move || {
            renderer.render(&context, prepared_sources, writer)
        })
        .await
        .unwrap()
        .unwrap();

        let final_position = final_writer.stream_position().unwrap();
        assert!(final_position > 0, "The writer should contain data.");

        final_writer.seek(SeekFrom::Start(0)).unwrap();
        let mut buffer = Vec::new();
        final_writer.read_to_end(&mut buffer).unwrap();

        let pdf_content = String::from_utf8_lossy(&buffer);
        assert!(
            pdf_content.starts_with("%PDF-1.7"),
            "Output should be a PDF file."
        );
    }
}
