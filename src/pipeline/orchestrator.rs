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

    /// Asynchronously generates a document by delegating to the configured strategy.
    ///
    /// The core generation logic is executed on a blocking thread pool via
    /// `task::spawn_blocking` to avoid stalling the Tokio runtime, as the strategies
    /// perform synchronous I/O and CPU-bound work.
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

    #[test]
    fn hybrid_strategy_with_toc_and_links_works() {
        let _ = env_logger::builder().is_test(true).try_init();
        // 1. Create a template with a heading, a link to it, and a TOC.
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
            .block_on(pipeline.generate_to_writer_async(data.into_iter(), writer))
            .unwrap();

        // 4. Verify the output
        let pdf_bytes = final_writer.into_inner();

        let doc_result = lopdf::Document::load_mem(&pdf_bytes);
        if let Err(e) = &doc_result {
            let debug_path = std::env::temp_dir().join("failed_hybrid_test.pdf");
            if !pdf_bytes.is_empty() {
                std::fs::write(&debug_path, &pdf_bytes).expect("Failed to write debug PDF");
                panic!("Failed to parse generated PDF: {}. Debug file written to {}", e, debug_path.display());
            } else {
                panic!("Failed to parse generated PDF: {}. The generated file was empty.", e);
            }
        }
        let doc = doc_result.unwrap();


        let catalog = doc.get_dictionary(doc.trailer.get(b"Root").unwrap().as_reference().unwrap()).unwrap();
        assert!(catalog.has(b"Outlines"), "PDF should have an Outlines dictionary for the TOC.");
        assert_eq!(catalog.get(b"PageMode").unwrap(), &lopdf::Object::Name(b"UseOutlines".to_vec()));

        // TOC page, Link page, Heading page.
        let pages = doc.get_pages();
        if pages.len() != 3 {
            let debug_path = std::env::temp_dir().join("failed_hybrid_test.pdf");
            std::fs::write(&debug_path, &pdf_bytes).expect("Failed to write debug PDF");
            eprintln!("\n--- TEST FAILURE DEBUG INFO ---");
            eprintln!("The generated PDF had an incorrect number of pages.");
            eprintln!("Please check the logs for '[WORKER-X] Finished paginating sequence' and '[HYBRID] Assembling final document'.");
            eprintln!("The expected layout is: 1 ToC page + 2 Body pages.");
            eprintln!("The worker should paginate the body into 2 pages.");
            eprintln!("--- END TEST FAILURE DEBUG INFO ---\n");
            panic!(
                "Expected 3 pages in the document, but found {}. Debug file written to {}",
                pages.len(),
                debug_path.display()
            );
        }
        assert_eq!(pages.len(), 3, "Expected 3 pages in the document");

        // The link is on the second page (index 1)
        let page2_id = pages.values().copied().collect::<Vec<_>>()[1];
        let page2_dict = doc.get_object(page2_id).unwrap().as_dict().unwrap();
        assert!(page2_dict.has(b"Annots"), "Page 2 should have an Annots array for the hyperlink.");

        let annots_arr = page2_dict.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annots_arr.len(), 1);
        let annot_dict = doc.get_object(annots_arr[0].as_reference().unwrap()).unwrap().as_dict().unwrap();
        assert_eq!(annot_dict.get(b"Subtype").unwrap(), &lopdf::Object::Name(b"Link".to_vec()));
    }
}