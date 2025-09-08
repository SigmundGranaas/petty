use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::parser::Parser;
use crate::render::pdf::PdfDocumentRenderer;
use crate::render::DocumentRenderer;
use crate::stylesheet::Stylesheet;
use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext};
use serde_json::Value;
use std::fs::File;
use std::io;
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
    pub fn new(stylesheet: Stylesheet) -> Self {
        let mut template_engine = Handlebars::new();
        template_engine.set_strict_mode(true);
        template_engine.register_helper("formatCurrency", Box::new(format_currency_helper));

        DocumentPipeline {
            stylesheet,
            template_engine,
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
                let event_parser = Parser::new(&self.stylesheet, &self.template_engine);
                event_parser.parse(data, &mut layout_processor)?;
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
        let file = File::create(path)?;
        self.generate_to_writer(data, file)
    }
}

/// A builder for configuring and creating a `DocumentPipeline`.
/// This is the main entry point for using the library.
pub struct PipelineBuilder {
    stylesheet: Option<Stylesheet>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        PipelineBuilder { stylesheet: None }
    }

    pub fn with_stylesheet_json(mut self, json: &str) -> Result<Self, PipelineError> {
        self.stylesheet = Some(Stylesheet::from_json(json)?);
        Ok(self)
    }

    pub fn build(self) -> Result<DocumentPipeline, PipelineError> {
        let stylesheet = self
            .stylesheet
            .ok_or_else(|| PipelineError::StylesheetError("No stylesheet provided".to_string()))?;
        let generator = DocumentPipeline::new(stylesheet);
        Ok(generator)
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}