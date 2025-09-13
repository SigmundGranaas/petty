// src/pipeline.rs
use crate::error::{PipelineError, RenderError};
use crate::idf::LayoutUnit;
use crate::layout::LayoutEngine;
use crate::parser::json_processor::JsonTemplateParser;
use crate::parser::processor::TemplateProcessor;
use crate::parser::xslt::XsltTemplateParser;
use crate::render::pdf::PdfDocumentRenderer;
use crate::render::DocumentRenderer;
use crate::stylesheet::Stylesheet;
use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::task;

/// The main document generation pipeline orchestrator.
pub struct DocumentPipeline {
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
    template_language: TemplateLanguage,
}

/// An enum holding the configuration for the chosen template language.
#[derive(Clone)]
pub enum TemplateLanguage {
    Json,
    Xslt { xslt_content: String },
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
    /// Creates a new pipeline from its constituent parts.
    pub fn new(stylesheet: Stylesheet, template_language: TemplateLanguage) -> Self {
        let mut template_engine = Handlebars::new();
        template_engine.set_strict_mode(true);
        template_engine.register_helper("formatCurrency", Box::new(format_currency_helper));

        DocumentPipeline {
            stylesheet,
            template_engine,
            template_language,
        }
    }

    /// Generates the document asynchronously and writes it to the provided stream.
    /// This method implements the concurrent pipeline architecture.
    pub async fn generate_to_writer_async<W: io::Write + Send + 'static>(
        &self,
        data: &Value,
        writer: W,
    ) -> Result<(), PipelineError> {
        // A channel to send fully parsed sequence trees from the parser task to the layout task.
        let (tx, mut rx) = mpsc::channel::<Result<LayoutUnit, PipelineError>>(32);

        // --- PARSER TASK (Producer) ---
        // This task runs on the Tokio runtime and sends `LayoutUnit`s asynchronously.
        let producer_stylesheet = self.stylesheet.clone();
        let producer_template_engine = self.template_engine.clone();
        let producer_template_language = self.template_language.clone();
        let data = data.clone();

        let producer_handle = task::spawn(async move {
            let mut processor: Box<dyn TemplateProcessor> = match producer_template_language {
                TemplateLanguage::Json => Box::new(JsonTemplateParser::new(
                    producer_stylesheet,
                    producer_template_engine,
                )),
                TemplateLanguage::Xslt { xslt_content } => Box::new(XsltTemplateParser::new(
                    xslt_content,
                    producer_stylesheet,
                    producer_template_engine,
                )),
            };

            let iter = match processor.process(&data) {
                Ok(it) => it,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            };

            for layout_unit_result in iter {
                if tx.send(layout_unit_result).await.is_err() {
                    // Receiver has been dropped, so we can stop.
                    break;
                }
            }
        });

        // --- LAYOUT & RENDER TASK (Consumer) ---
        // This task runs in a dedicated OS thread to handle the `!Send` renderer.
        let consumer_stylesheet = self.stylesheet.clone();
        let consumer_template_engine = self.template_engine.clone();

        let consumer_handle = thread::spawn(move || {
            let layout_engine = LayoutEngine::new(consumer_stylesheet);
            // The !Send renderer is created and used only within this thread.
            let mut renderer = PdfDocumentRenderer::new(layout_engine.clone())?;
            renderer.begin_document()?;

            // Pull items from the channel using the blocking receive method.
            while let Some(layout_unit_result) = rx.blocking_recv() {
                let layout_unit = layout_unit_result?;
                let page_iterator = layout_engine.paginate_tree(&layout_unit)?;

                for page_elements in page_iterator {
                    renderer.render_page(&layout_unit.context, page_elements)?;
                }
            }

            // The renderer's finalize method is blocking, which is fine in a std::thread.
            renderer.finalize(writer, &consumer_template_engine)?;

            Ok::<(), PipelineError>(())
        });

        // Wait for the producer to finish.
        producer_handle.await.map_err(|e| {
            PipelineError::TemplateParseError(format!("Producer task panicked: {}", e))
        })?;

        // Wait for the consumer to finish and handle its result.
        match consumer_handle.join() {
            Ok(result) => result,
            Err(_) => Err(PipelineError::RenderError(RenderError::Aborted)),
        }
    }

    /// Generates the document and saves it to the specified file path.
    /// This is a convenience wrapper that sets up a Tokio runtime.
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

        // Run the async function directly on the multi-threaded runtime.
        rt.block_on(self.generate_to_writer_async(data, file))
    }
}

/// A builder for configuring and creating a `DocumentPipeline`.
/// This is the main entry point for using the library.
#[derive(Default)]
pub struct PipelineBuilder {
    stylesheet: Option<Stylesheet>,
    template_language: Option<TemplateLanguage>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Configures the builder from a stylesheet JSON string.
    pub fn with_stylesheet_json(mut self, json: &str) -> Result<Self, PipelineError> {
        let stylesheet = Stylesheet::from_json(json)?;
        self.stylesheet = Some(stylesheet);
        self.template_language = Some(TemplateLanguage::Json);
        Ok(self)
    }

    /// Configures the builder from a stylesheet JSON file.
    pub fn with_stylesheet_file<P: AsRef<Path>>(self, path: P) -> Result<Self, PipelineError> {
        let json_str = fs::read_to_string(path)?;
        self.with_stylesheet_json(&json_str)
    }

    /// Configures the builder from a self-contained XSLT template file.
    pub fn with_xslt_template_file<P: AsRef<Path>>(
        mut self,
        path: P,
    ) -> Result<Self, PipelineError> {
        let xslt_content = fs::read_to_string(path)?;
        let stylesheet = Stylesheet::from_xslt(&xslt_content)?;
        self.stylesheet = Some(stylesheet);
        self.template_language = Some(TemplateLanguage::Xslt { xslt_content });
        Ok(self)
    }

    /// Builds the final `DocumentPipeline`.
    pub fn build(self) -> Result<DocumentPipeline, PipelineError> {
        let stylesheet = self.stylesheet.ok_or_else(|| {
            PipelineError::StylesheetError("No stylesheet or template provided".to_string())
        })?;
        let language = self.template_language.ok_or_else(|| {
            PipelineError::StylesheetError("Template language could not be determined".to_string())
        })?;
        let generator = DocumentPipeline::new(stylesheet, language);
        Ok(generator)
    }
}