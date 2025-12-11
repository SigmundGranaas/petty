use crate::error::PipelineError;
use crate::pipeline::api::PreparedDataSources;
use crate::pipeline::config::PdfBackend;
use crate::pipeline::concurrency::{
    producer_task, run_in_order_streaming_consumer, spawn_workers,
};
use crate::pipeline::context::PipelineContext;
use crate::pipeline::renderer::RenderingStrategy;
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::DocumentRenderer;
use log::{info, warn};
use std::io::{Seek, Write};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task;

// Need to import LayoutEngine for the consumer stage.
use crate::core::layout::LayoutEngine;

/// A rendering strategy that streams the document directly to the output.
#[derive(Clone)]
pub struct SinglePassStreamingRenderer {
    pdf_backend: PdfBackend,
}

impl SinglePassStreamingRenderer {
    pub fn new(pdf_backend: PdfBackend) -> Self {
        Self { pdf_backend }
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
            warn!("SinglePassStreamingRenderer received Document metadata but cannot use it. The metadata will be ignored.");
        }
        if sources.body_artifact.is_some() {
            warn!("SinglePassStreamingRenderer received a pre-rendered body artifact but cannot use it. The artifact will be ignored.");
        }

        if !matches!(self.pdf_backend, PdfBackend::Lopdf | PdfBackend::LopdfParallel) {
            return Err(PipelineError::Config(
                "SinglePassStreamingRenderer only supports the 'Lopdf' or 'LopdfParallel' backend."
                    .into(),
            ));
        }

        let num_layout_threads = num_cpus::get().saturating_sub(1).clamp(2, 6);
        let channel_buffer_size = num_layout_threads;

        let max_in_flight = num_layout_threads + 2;
        let semaphore = Arc::new(Semaphore::new(max_in_flight));

        info!(
            "Starting Single-Pass Streaming pipeline with {} layout workers (Max in-flight: {}).",
            num_layout_threads, max_in_flight
        );

        let (tx1, rx1) = async_channel::bounded(channel_buffer_size);
        let (tx2, rx2) = async_channel::bounded(channel_buffer_size);

        let producer = task::spawn(producer_task(sources.data_iterator, tx1, semaphore.clone()));
        let workers = spawn_workers(num_layout_threads, context, rx1, tx2);

        // --- Consumer Stage ---
        info!("[CONSUMER] Started in-order streaming consumer. Awaiting laid-out sequences.");
        let final_layout_engine = LayoutEngine::new(&context.font_library, context.cache_config);
        let final_stylesheet = context.compiled_template.stylesheet();

        // Pass Arc<Stylesheet> correctly
        let mut renderer = LopdfRenderer::new(final_layout_engine, final_stylesheet.clone())?;
        renderer.begin_document(writer)?;

        let (page_width, page_height) =
            renderer.stylesheet.get_default_page_layout().size.dimensions_pt();

        let (all_page_ids, _) = run_in_order_streaming_consumer(
            rx2,
            &mut renderer,
            page_width,
            page_height,
            false,
            semaphore
        )?;

        let writer = Box::new(renderer).finish(all_page_ids)?;

        producer.abort();
        for worker in workers {
            worker.abort();
        }
        info!("[CONSUMER] Finished streaming.");
        Ok(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::fonts::SharedFontLibrary;
    use crate::parser::json::processor::JsonParser;
    use crate::parser::processor::TemplateParser;
    use crate::pipeline::provider::passthrough::PassThroughProvider;
    use crate::pipeline::provider::DataSourceProvider;
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
        let parser = JsonParser;
        let features = parser.parse(&template_str, PathBuf::new()).unwrap();
        let library = SharedFontLibrary::new();
        library.load_fallback_font();

        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(features.role_templates),
            font_library: Arc::new(library),
            cache_config: Default::default(),
        };

        let provider = PassThroughProvider;
        let renderer = SinglePassStreamingRenderer::new(PdfBackend::Lopdf);

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

        let final_position = final_writer.seek(SeekFrom::Current(0)).unwrap();
        assert!(final_position > 0, "The writer should contain data.");

        final_writer.seek(SeekFrom::Start(0)).unwrap();
        let mut buffer = Vec::new();
        final_writer.read_to_end(&mut buffer).unwrap();

        let pdf_content = String::from_utf8_lossy(&buffer);
        assert!(pdf_content.starts_with("%PDF-1.7"), "Output should be a PDF file.");
    }
}