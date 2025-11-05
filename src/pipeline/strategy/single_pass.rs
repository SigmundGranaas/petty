use super::{PipelineContext};
use crate::core::layout::LayoutEngine;
use crate::error::PipelineError;
use crate::pipeline::config::PdfBackend;
use crate::pipeline::strategy::two_pass::{producer_task, spawn_workers, run_in_order_streaming_consumer};
use crate::render::lopdf_renderer::LopdfRenderer;
use crate::render::DocumentRenderer;
use log::{info};
use serde_json::Value;
use std::io::{Seek, Write};
use tokio::task;
use std::sync::Arc;

#[derive(Clone)]
pub struct SinglePassStreamingStrategy {
    pdf_backend: PdfBackend,
}

impl SinglePassStreamingStrategy {
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
        I: Iterator<Item=Value> + Send + 'static,
    {
        if !matches!(self.pdf_backend, PdfBackend::Lopdf | PdfBackend::LopdfParallel) {
            return Err(PipelineError::Config(
                "SinglePassStreamingStrategy only supports the 'Lopdf' or 'LopdfParallel' backend.".into()
            ));
        }

        let num_layout_threads = num_cpus::get().saturating_sub(1).max(4);
        let channel_buffer_size = num_layout_threads;

        info!("Starting Single-Pass Streaming pipeline with {} layout workers.", num_layout_threads);

        let (tx1, rx1) = async_channel::bounded(channel_buffer_size);
        let (tx2, rx2) = async_channel::bounded(channel_buffer_size);

        let producer = task::spawn(producer_task(data_iterator, tx1));
        let workers = spawn_workers(num_layout_threads, context, rx1, tx2);

        // --- Consumer Stage ---
        info!("[CONSUMER] Started in-order streaming consumer. Awaiting laid-out sequences.");
        let final_layout_engine = LayoutEngine::new(Arc::clone(&context.font_manager));
        let final_stylesheet = context.compiled_template.stylesheet();

        let mut renderer = LopdfRenderer::new(final_layout_engine, final_stylesheet.clone())?;
        renderer.begin_document(writer)?;

        let (page_width, page_height) = renderer.stylesheet.get_default_page_layout().size.dimensions_pt();

        // The consumer now processes sequences in strict order, buffering only when necessary
        // to fill gaps, and writes to the stream as soon as a contiguous chunk is available.
        let (all_page_ids, _) = run_in_order_streaming_consumer(
            rx2,
            &mut renderer,
            page_width,
            page_height,
            false, // No analysis needed for single pass
        )?;

        let writer = Box::new(renderer).finish(all_page_ids)?;

        producer.abort();
        for worker in workers { worker.abort(); }
        info!("[CONSUMER] Finished streaming.");
        Ok(writer)
    }
}