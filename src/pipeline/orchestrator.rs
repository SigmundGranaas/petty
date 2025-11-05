// src/pipeline/orchestrator.rs
// src/pipeline/orchestrator.rs
use crate::error::PipelineError;
use serde_json::Value;
use std::io::{self, Seek, Write};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio::task;
use super::strategy::{GenerationStrategy, PipelineContext};

/// The main document generation pipeline.
/// This struct acts as a simple facade that holds the selected generation
/// strategy and delegates the execution to it.
pub struct DocumentPipeline {
    strategy: GenerationStrategy,
    context: Arc<PipelineContext>,
}

impl DocumentPipeline {
    /// This constructor is intended to be called by the `PipelineBuilder`.
    pub(super) fn new(
        strategy: GenerationStrategy,
        context: Arc<PipelineContext>,
    ) -> Self {
        Self { strategy, context }
    }

    /// Asynchronously generates a document by delegating to the configured strategy.
    ///
    /// The core generation logic is executed on a blocking thread pool via
    /// `task::spawn_blocking` to avoid stalling the Tokio runtime, as the

    /// strategies perform synchronous I/O and CPU-bound work.
    ///
    /// Note: The writer must implement `Seek` because the PDF format requires a
    /// cross-reference table at the end of the file, which points to the byte offsets
    /// of all objects written earlier.
    pub async fn generate_to_writer_async<W, I>(
        &self,
        data_iterator: I,
        writer: W,
    ) -> Result<W, PipelineError>
    where
        W: io::Write + io::Seek + Send + 'static,
        I: Iterator<Item = Value> + Send + 'static,
    {
        let strategy = self.strategy.clone();
        let context = Arc::clone(&self.context);

        task::spawn_blocking(move || -> Result<W, PipelineError> {
            match strategy {
                GenerationStrategy::SinglePass(s) => {
                    // Pass the writer directly for true streaming.
                    s.generate(context.as_ref(), data_iterator, writer)
                }
                GenerationStrategy::TwoPass(s) => {
                    let data: Vec<Value> = data_iterator.collect();
                    s.generate(context.as_ref(), data.into_iter(), writer)
                }
                GenerationStrategy::Hybrid(s) => {
                    s.generate(context.as_ref(), data_iterator, writer)
                }
            }
        })
            .await
            .unwrap() // Propagate panics from the blocking task
    }

    /// A convenience method to generate a document to a file path.
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

        // block_on runs the future to completion. The returned writer is dropped.
        rt.block_on(self.generate_to_writer_async(data_iterator, writer))?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::builder::PipelineBuilder;
    use crate::pipeline::config::{GenerationMode, PdfBackend};
    use crate::templating::Template;
    use serde_json::json;
    use std::io::{Cursor, Read, SeekFrom};

    #[test]
    fn single_pass_strategy_writes_incrementally() {
        // This test verifies that the SinglePass strategy with the orchestrator
        // writes data to the provided writer without buffering the whole file in memory.
        let template_json = json!({
            "stylesheet": { "default": {} },
            "body": [ { "type": "paragraph", "children": [ { "type": "text", "content": "Hello {{name}}" } ] } ]
        });
        let template = Template::from_json(template_json).unwrap();

        let pipeline = PipelineBuilder::new()
            .with_template_object(template)
            .unwrap()
            .with_generation_mode(GenerationMode::ForceSinglePass)
            .with_pdf_backend(PdfBackend::Lopdf)
            .build()
            .unwrap();

        let data = vec![json!({"name": "World"})];
        let writer = Cursor::new(Vec::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut final_writer = rt
            .block_on(pipeline.generate_to_writer_async(data.into_iter(), writer))
            .unwrap();

        // After generation, check if the writer contains a valid PDF header and some content.
        let final_position = final_writer.seek(SeekFrom::Current(0)).unwrap();
        assert!(final_position > 0, "The writer position should be at the end of the file, not zero.");

        final_writer.seek(SeekFrom::Start(0)).unwrap();
        let mut buffer = Vec::new();
        final_writer.read_to_end(&mut buffer).unwrap();

        let pdf_content = String::from_utf8_lossy(&buffer);
        assert!(pdf_content.starts_with("%PDF-1.7"), "Output should be a PDF file.");
        assert!(pdf_content.contains("/Type /Page"), "Output should contain PDF page objects.");
        assert!(pdf_content.contains("Hello World"), "Output should contain rendered text.");
        assert!(pdf_content.trim_end().ends_with("%%EOF"), "Output should be a complete PDF file.");
    }
}