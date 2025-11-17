// src/pipeline/orchestrator.rs
use crate::error::PipelineError;
use crate::pipeline::context::PipelineContext;
use crate::pipeline::provider::{DataSourceProvider, Provider};
use crate::pipeline::renderer::{Renderer, RenderingStrategy};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio::task;

/// The main document generation pipeline.
/// This struct holds the configured provider and renderer and orchestrates
/// the two-stage process: data preparation, then rendering.
pub struct DocumentPipeline {
    provider: Provider,
    renderer: Renderer,
    context: Arc<PipelineContext>,
}

impl DocumentPipeline {
    /// This constructor is intended to be called by the `PipelineBuilder`.
    pub(super) fn new(
        provider: Provider,
        renderer: Renderer,
        context: Arc<PipelineContext>,
    ) -> Self {
        Self { provider, renderer, context }
    }

    /// Asynchronously generates a document from any data source iterator.
    ///
    /// The pipeline abstracts away the complexities of streaming vs. buffering.
    /// If the configured `DataSourceProvider` needs to buffer data (e.g., to
    /// perform an analysis pass), it will do so, potentially to a temporary file
    /// to keep memory usage low.
    pub async fn generate<W, I>(&self, data_iterator: I, writer: W) -> Result<W, PipelineError>
    where
        W: io::Write + io::Seek + Send + 'static,
        I: Iterator<Item = Value> + Send + 'static,
    {
        // Clone the lightweight enums and the Arc to move them into the blocking task.
        let provider = self.provider.clone();
        let renderer = self.renderer.clone();
        let context_clone = Arc::clone(&self.context);

        task::spawn_blocking(move || {
            let sources = provider.provide(&context_clone, data_iterator)?;
            renderer.render(&context_clone, sources, writer)
        })
            .await
            .unwrap() // Propagate panics from the spawned task
    }

    /// A convenience method to generate a document to a file path from a dataset in memory.
    pub fn generate_to_file<P: AsRef<Path>>(
        &self,
        data: Vec<Value>,
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

        rt.block_on(self.generate(data.into_iter(), writer))?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::builder::PipelineBuilder;
    use crate::pipeline::config::{GenerationMode, PdfBackend};
    use serde_json::json;
    use std::io::{Cursor, Read, Seek, SeekFrom};

    #[tokio::test]
    async fn test_streaming_pipeline_writes_to_output() {
        // This test verifies that the fast path (PassThroughProvider -> SinglePassStreamingRenderer)
        // works correctly.
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

        let pipeline = PipelineBuilder::new()
            .with_template_source(&template_str, "json")
            .unwrap()
            // Using Auto should select the streaming pipeline for this simple template
            .with_generation_mode(GenerationMode::Auto)
            .with_pdf_backend(PdfBackend::Lopdf)
            .build()
            .unwrap();

        let data = vec![json!({"name": "World"})];
        let writer = Cursor::new(Vec::new());

        let mut final_writer = pipeline.generate(data.into_iter(), writer).await.unwrap();

        let final_position = final_writer.seek(SeekFrom::Current(0)).unwrap();
        assert!(final_position > 0, "The writer should contain data.");

        final_writer.seek(SeekFrom::Start(0)).unwrap();
        let mut buffer = Vec::new();
        final_writer.read_to_end(&mut buffer).unwrap();

        let pdf_content = String::from_utf8_lossy(&buffer);
        assert!(pdf_content.starts_with("%PDF-1.7"), "Output should be a PDF file.");
        assert!(pdf_content.contains("Hello World"), "Output should contain rendered text.");
    }

    #[tokio::test]
    async fn test_metadata_pipeline_with_links_and_outlines() {
        // This test verifies the advanced path (MetadataGeneratingProvider -> ComposingRenderer)
        // by checking for link annotations and PDF outlines.
        let _ = env_logger::builder().is_test(true).try_init();

        let template_json = json!({
            "_stylesheet": {
                "defaultPageMaster": "default",
                "pageMasters": { "default": { "size": "A4", "margins": "1cm" } },
                "styles": { "default": { "font-family": "Helvetica" } }
            },
            "_template": { "type": "Block", "children": [
                 { "type": "Paragraph", "children": [ { "type": "Hyperlink", "href": "#h1", "children": [ { "type": "Text", "content": "Link to Heading" } ] } ] },
                // This legacy <TableOfContents/> tag forces feature detection to select the metadata pipeline
                { "type": "TableOfContents" },
                { "type": "PageBreak" },
                { "type": "Heading", "level": 2, "id": "h1", "children": [ { "type": "Text", "content": "My Heading" } ] }
            ]}
        });
        let template_str = serde_json::to_string(&template_json).unwrap();

        let pipeline = PipelineBuilder::new()
            .with_template_source(&template_str, "json").unwrap()
            // Auto should detect the ToC and select the metadata pipeline
            .with_generation_mode(GenerationMode::Auto)
            .with_pdf_backend(PdfBackend::Lopdf).build().unwrap();

        let data = vec![json!({})];
        let writer = Cursor::new(Vec::new());
        let final_writer = pipeline.generate(data.into_iter(), writer).await.unwrap();

        let pdf_bytes = final_writer.into_inner();
        assert!(!pdf_bytes.is_empty(), "PDF should not be empty");
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("Failed to parse generated PDF");

        // 1. Check for Outlines (from ToC entries)
        let catalog = doc.get_dictionary(doc.trailer.get(b"Root").unwrap().as_reference().unwrap()).unwrap();
        assert!(catalog.has(b"Outlines"), "PDF should have an Outlines dictionary for the bookmarks.");

        // 2. Check for Link Annotation
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2);
        let page1_id = pages.get(&1).unwrap();
        let page1_dict = doc.get_object(*page1_id).unwrap().as_dict().unwrap();
        assert!(page1_dict.has(b"Annots"), "Page 1 should have an Annots array for the hyperlink.");
    }

    #[test]
    fn test_generate_to_file_creates_file() {
        // Simple test to ensure the convenience method works.
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.pdf");

        let template_json = json!({
            "_stylesheet": { "defaultPageMaster": "default", "pageMasters": { "default": { "size": "A4", "margins": "1cm" } } },
            "_template": { "type": "Paragraph", "children": [ { "type": "Text", "content": "test" } ] }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();

        let pipeline = PipelineBuilder::new()
            .with_template_source(&template_str, "json").unwrap()
            .build().unwrap();

        let data = vec![json!({})];
        pipeline.generate_to_file(data, &output_path).unwrap();

        assert!(output_path.exists());
        let metadata = fs::metadata(&output_path).unwrap();
        assert!(metadata.len() > 0);
    }
}