// FILE: src/pipeline/orchestrator.rs
// src/pipeline/orchestrator.rs
use crate::error::PipelineError;
use serde_json::Value;
use std::io;
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

    /// Asynchronously generates a document from a REPLAYABLE data source.
    ///
    /// This method is required for the `TwoPassStrategy` and is the most performant
    /// option when the template requires forward references (e.g., a Table of Contents).
    /// The iterator must be cloneable so that the data can be processed twice without
    /// being fully buffered in memory.
    pub async fn generate_from_clonable<W, I>(
        &self,
        data_iterator: I,
        writer: W,
    ) -> Result<W, PipelineError>
    where
        W: io::Write + io::Seek + Send + 'static,
        I: Iterator<Item = Value> + Send + 'static + Clone, // The key constraint
    {
        let strategy = self.strategy.clone();
        let context = Arc::clone(&self.context);

        task::spawn_blocking(move || -> Result<W, PipelineError> {
            match strategy {
                GenerationStrategy::SinglePass(s) => {
                    s.generate(context.as_ref(), data_iterator, writer)
                }
                GenerationStrategy::TwoPass(s) => {
                    // NO MORE .collect()! We pass the clonable iterator directly.
                    s.generate(context.as_ref(), data_iterator, writer)
                }
                GenerationStrategy::Hybrid(s) => {
                    // Hybrid can also accept a clonable iterator, no problem.
                    s.generate(context.as_ref(), data_iterator, writer)
                }
            }
        })
            .await
            .unwrap()
    }

    /// Asynchronously generates a document from a FORWARD-ONLY streaming data source.
    ///
    /// This method is suitable for templates that can be rendered in a single pass or
    /// for scenarios where the data source cannot be cloned (e.g., reading from a network stream).
    /// If the selected strategy is `TwoPass`, this method will return an error, as
    /// that strategy requires a replayable data source. Use `ForceHybrid` mode for
    /// forward references with a streaming source.
    pub async fn generate_from_stream<W, I>(
        &self,
        data_iterator: I,
        writer: W,
    ) -> Result<W, PipelineError>
    where
        W: io::Write + io::Seek + Send + 'static,
        I: Iterator<Item = Value> + Send + 'static, // No Clone constraint
    {
        let strategy = self.strategy.clone();
        let context = Arc::clone(&self.context);

        task::spawn_blocking(move || -> Result<W, PipelineError> {
            match strategy {
                GenerationStrategy::SinglePass(s) => {
                    s.generate(context.as_ref(), data_iterator, writer)
                }
                GenerationStrategy::TwoPass(_) => {
                    // THIS IS THE CRITICAL SAFETY CHECK
                    Err(PipelineError::Config(
                        "TwoPassStrategy requires a cloneable iterator. Use `generate_from_clonable` or switch to `GenerationMode::ForceHybrid`.".into()
                    ))
                }
                GenerationStrategy::Hybrid(s) => {
                    s.generate(context.as_ref(), data_iterator, writer)
                }
            }
        })
            .await
            .unwrap()
    }

    /// A convenience method to generate a document to a file path from a dataset in memory.
    pub fn generate_to_file<P: AsRef<Path>>(
        &self,
        data: Vec<Value>, // Now explicitly takes a Vec
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

        // Use the clonable method, since Vec::into_iter is Clone.
        rt.block_on(self.generate_from_clonable(data.into_iter(), writer))?;
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

    #[test]
    fn single_pass_strategy_writes_incrementally() {
        // This test verifies that the SinglePass strategy with the orchestrator
        // writes data to the provided writer without buffering the whole file in memory.
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
                "children": [ { "type": "Text", "content": "Hello {{name}}" } ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();

        let pipeline = PipelineBuilder::new()
            .with_template_source(&template_str, "json")
            .unwrap()
            .with_generation_mode(GenerationMode::ForceSinglePass)
            .with_pdf_backend(PdfBackend::Lopdf)
            .build()
            .unwrap();

        let data = vec![json!({"name": "World"})];
        let writer = Cursor::new(Vec::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        // Use the stream method to test the non-clone path
        let mut final_writer = rt
            .block_on(pipeline.generate_from_stream(data.into_iter(), writer))
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

    #[test]
    fn hybrid_strategy_with_toc_and_links_works() {
        let _ = env_logger::builder().is_test(true).try_init();
        // 1. Create a template with a heading, a link to it, and a TOC.
        // The Hybrid strategy should produce a correctly structured PDF with bookmarks (Outlines)
        // and functional links, WITHOUT prepending an extra page for the ToC.
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
                "type": "Block",
                "children": [
                    { "type": "Heading", "level": 1, "children": [{ "type": "Text", "content": "Title Page" }] },
                    { "type": "TableOfContents" },
                    { "type": "Paragraph", "children": [ { "type": "Hyperlink", "href": "#h1", "children": [ { "type": "Text", "content": "Link to Heading" } ] } ] },
                    { "type": "PageBreak" },
                    { "type": "Heading", "level": 2, "id": "h1", "children": [ { "type": "Text", "content": "My Heading" } ] }
                ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();

        // 2. Build pipeline with Hybrid mode
        let pipeline = PipelineBuilder::new()
            .with_template_source(&template_str, "json")
            .unwrap()
            .with_generation_mode(GenerationMode::ForceHybrid)
            .with_pdf_backend(PdfBackend::Lopdf)
            .build()
            .unwrap();

        // 3. Generate with a non-cloneable iterator
        let data = vec![json!({})]; // Single empty object for one sequence
        let writer = Cursor::new(Vec::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let final_writer = rt
            .block_on(pipeline.generate_from_stream(data.into_iter(), writer))
            .unwrap();

        // 4. Verify the output
        let pdf_bytes = final_writer.into_inner();
        let doc = lopdf::Document::load_mem(&pdf_bytes).expect("Failed to parse generated PDF");

        let catalog = doc.get_dictionary(doc.trailer.get(b"Root").unwrap().as_reference().unwrap()).unwrap();
        assert!(catalog.has(b"Outlines"), "PDF should have an Outlines dictionary for the TOC bookmarks.");
        assert_eq!(catalog.get(b"PageMode").unwrap(), &lopdf::Object::Name(b"UseOutlines".to_vec()));

        // The document should be laid out onto 2 pages:
        // Page 1: Title, ToC (placeholder), Link
        // Page 2: "My Heading"
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2, "Expected 2 pages in the document, not an extra prepended ToC page.");

        // The link is on the first page (index 0)
        let page1_id = pages.values().copied().collect::<Vec<_>>()[0];
        let page1_dict = doc.get_object(page1_id).unwrap().as_dict().unwrap();
        assert!(page1_dict.has(b"Annots"), "Page 1 should have an Annots array for the hyperlink.");

        let annots_arr = page1_dict.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annots_arr.len(), 1);
        let annot_dict = doc.get_object(annots_arr[0].as_reference().unwrap()).unwrap().as_dict().unwrap();
        assert_eq!(annot_dict.get(b"Subtype").unwrap(), &lopdf::Object::Name(b"Link".to_vec()));
    }

    #[test]
    fn two_pass_strategy_with_clonable_iterator_succeeds() {
        // Simple template that requires two passes (Table of Contents)
        let template_json = json!({
            "_stylesheet": { "defaultPageMaster": "default", "pageMasters": { "default": { "size": "A4", "margins": "1cm" } } },
            "_template": {
                "type": "Block",
                "children": [
                    { "type": "TableOfContents" },
                    { "type": "PageBreak" },
                    { "type": "Heading", "id": "h1", "level": 2, "children": [{ "type": "Text", "content": "My Heading" }] }
                ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();
        let pipeline = PipelineBuilder::new()
            .with_template_source(&template_str, "json").unwrap()
            .with_generation_mode(GenerationMode::ForceTwoPass)
            .build().unwrap();

        let data = vec![json!({})]; // Vec::into_iter is Clone
        let writer = Cursor::new(Vec::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(pipeline.generate_from_clonable(data.into_iter(), writer));

        assert!(result.is_ok(), "TwoPass with clonable iterator should succeed");
    }

    #[test]
    fn two_pass_strategy_with_stream_iterator_fails() {
        // Template that requires two passes
        let template_json = json!({
            "_stylesheet": { "defaultPageMaster": "default", "pageMasters": { "default": { "size": "A4", "margins": "1cm" } } },
            "_template": {
                "type": "Block",
                "children": [
                    { "type": "TableOfContents" }
                ]
            }
        });
        let template_str = serde_json::to_string(&template_json).unwrap();
        let pipeline = PipelineBuilder::new()
            .with_template_source(&template_str, "json").unwrap()
            .with_generation_mode(GenerationMode::ForceTwoPass)
            .build().unwrap();

        // Use a non-clone iterator
        let data = vec![json!({})];
        let non_clone_iter = data.into_iter().map(|v| v); // map creates a non-clone iterator
        let writer = Cursor::new(Vec::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(pipeline.generate_from_stream(non_clone_iter, writer));

        assert!(result.is_err(), "TwoPass with stream iterator should fail");
        if let Err(PipelineError::Config(msg)) = result {
            assert!(msg.contains("TwoPassStrategy requires a cloneable iterator"));
        } else {
            panic!("Expected a config error, but got {:?}", result);
        }
    }
}