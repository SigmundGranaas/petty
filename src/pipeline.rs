// src/pipeline.rs
use crate::error::PipelineError;
use crate::idf::IDFEvent;
use crate::layout::StreamingLayoutProcessor;
use crate::parser::json_processor::JsonTemplateParser;
use crate::parser::processor::{LayoutProcessorProxy, TemplateProcessor};
use crate::parser::xslt::XsltTemplateParser;
use crate::render::pdf::PdfDocumentRenderer;
use crate::render::DocumentRenderer;
use crate::stylesheet::Stylesheet;
use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
use tokio::join;
use tokio::runtime::Builder;
use tokio::sync::mpsc;

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
    /// This is typically called by `PipelineBuilder`.
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

    pub async fn generate_to_writer_async<'a, W: io::Write + Send + 'static>(
        &'a self,
        data: &'a Value,
        writer: W,
    ) -> Result<(), PipelineError> {
        let (tx, mut rx) = mpsc::channel::<IDFEvent<'a>>(1024);

        // Define the producer as a future. It can borrow from `self` and `data`.
        let producer_fut = async {
            // `proxy` takes ownership of `tx`. When `proxy` is dropped at the end of this
            // future, the channel is closed, which will terminate the consumer's loop.
            let mut proxy = LayoutProcessorProxy::new(tx);
            match &self.template_language {
                TemplateLanguage::Json => {
                    let mut processor =
                        JsonTemplateParser::new(&self.stylesheet, &self.template_engine);
                    processor.process(data, &mut proxy).await
                }
                TemplateLanguage::Xslt { xslt_content } => {
                    let mut processor = XsltTemplateParser::new(
                        xslt_content,
                        &self.stylesheet,
                        self.template_engine.clone(),
                    );
                    processor.process(data, &mut proxy).await
                }
            }
        };

        // Define the consumer as a future.
        let consumer_fut = async {
            let renderer = PdfDocumentRenderer::new(&self.stylesheet)?;
            let mut layout_processor = StreamingLayoutProcessor::new(renderer, &self.stylesheet);

            while let Some(event) = rx.recv().await {
                layout_processor.process_event(event)?;
            }

            // Return the processor which owns the renderer, so it can be finalized.
            Ok::<_, PipelineError>(layout_processor)
        };

        // Run both futures concurrently. `join!` awaits both to complete.
        let (producer_result, consumer_result) = join!(producer_fut, consumer_fut);

        // Check for errors from both futures.
        producer_result?;
        let layout_processor = consumer_result?;

        // --- Finalization ---
        let renderer = layout_processor.into_renderer();
        renderer.finalize(writer, &self.template_engine)?;

        Ok(())
    }

    /// Generates the document and saves it to the specified file path.
    /// This is a convenience wrapper that sets up a Tokio runtime.
    pub fn generate_to_file<'a, P: AsRef<std::path::Path>>(
        &'a self,
        data: &'a Value,
        path: P,
    ) -> Result<(), PipelineError> {
        let file = std::fs::File::create(path)?;
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");
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

    /// Configures the builder from a stylesheet JSON string. This will use the JSON
    /// templating engine.
    pub fn with_stylesheet_json(mut self, json: &str) -> Result<Self, PipelineError> {
        let stylesheet = Stylesheet::from_json(json)?;
        self.stylesheet = Some(stylesheet);
        self.template_language = Some(TemplateLanguage::Json);
        Ok(self)
    }

    /// Configures the builder from a stylesheet JSON file. This will use the JSON
    /// templating engine.
    pub fn with_stylesheet_file<P: AsRef<Path>>(self, path: P) -> Result<Self, PipelineError> {
        let json_str = fs::read_to_string(path)?;
        self.with_stylesheet_json(&json_str)
    }

    /// Configures the builder from a self-contained XSLT template file.
    /// This template must contain `<petty:page-layout>` and `<xsl:attribute-set>`
    /// blocks which are used to configure the document's styles and layout.
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