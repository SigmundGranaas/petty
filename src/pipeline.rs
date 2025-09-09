use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
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
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct Metrics {
    pub total_time: Duration,
    pub stage_timings: Vec<(String, Duration)>,
}

impl Metrics {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn time_scope<F, R>(&mut self, name: &str, func: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = func();
        let duration = start.elapsed();
        self.stage_timings.push((name.to_string(), duration));
        result
    }
    pub fn report(&mut self) {
        self.total_time = self.stage_timings.iter().map(|(_, d)| *d).sum();
        println!("--- Performance Report ---");
        for (name, duration) in &self.stage_timings {
            println!("  - {}: {:.2}ms", name, duration.as_secs_f64() * 1000.0);
        }
        println!("--------------------------");
        println!(
            "Total Time: {:.2}ms",
            self.total_time.as_secs_f64() * 1000.0
        );
    }
}

/// The main document generation pipeline orchestrator.
pub struct DocumentPipeline {
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
    template_language: TemplateLanguage,
}

/// An enum holding the configuration for the chosen template language.
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

    /// Generates the document and streams it to the given writer.
    pub fn generate_to_writer<'a, W: io::Write>(
        &'a self,
        data: &'a Value,
        writer: W,
    ) -> Result<(), PipelineError> {
        let mut metrics = Metrics::new();
        let renderer = PdfDocumentRenderer::new(&self.stylesheet)?;

        let renderer = metrics.time_scope(
            "Event Parsing & Layout",
            || -> Result<PdfDocumentRenderer<'a>, PipelineError> {
                let mut layout_processor =
                    StreamingLayoutProcessor::new(renderer, &self.stylesheet);

                match &self.template_language {
                    TemplateLanguage::Json => {
                        let mut processor =
                            JsonTemplateParser::new(&self.stylesheet, &self.template_engine);
                        processor.process(data, &mut layout_processor)?;
                    }
                    TemplateLanguage::Xslt { xslt_content } => {
                        let mut processor = XsltTemplateParser::new(
                            xslt_content,
                            &self.stylesheet,
                            self.template_engine.clone(),
                        );
                        processor.process(data, &mut layout_processor)?;
                    }
                }
                Ok(layout_processor.into_renderer())
            },
        )?;

        metrics.time_scope("PDF Finalization", || {
            renderer.finalize(writer, &self.template_engine)
        })?;

        metrics.report();
        Ok(())
    }

    /// Generates the document and saves it to the specified file path.
    pub fn generate_to_file<'a, P: AsRef<std::path::Path>>(
        &'a self,
        data: &'a Value,
        path: P,
    ) -> Result<(), PipelineError> {
        let file = fs::File::create(path)?;
        self.generate_to_writer(data, file)
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