// src/generator.rs

use crate::errors::PipelineError;
use crate::events::EventParser;
use crate::renderer::PdfDocumentRenderer;
use crate::streaming_layout::StreamingLayoutProcessor;
use crate::stylesheet::Stylesheet;
use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext};
use serde_json::Value;
use std::time::{Duration, Instant};
use crate::DocumentRenderer;

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

pub struct DocumentGenerator {
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

impl DocumentGenerator {
    pub fn new(stylesheet: Stylesheet) -> Self {
        let mut template_engine = Handlebars::new();
        template_engine.set_strict_mode(true);
        template_engine
            .register_helper("formatCurrency", Box::new(format_currency_helper));

        DocumentGenerator {
            stylesheet,
            template_engine,
        }
    }

    pub fn generate(&self, data: &Value) -> Result<Vec<u8>, PipelineError> {
        let mut metrics = Metrics::new();
        let mut renderer = PdfDocumentRenderer::new(
            self.stylesheet.page.title.as_deref().unwrap_or("Document"),
            self.stylesheet.clone(),
        )?;

        let process_result =
            metrics.time_scope("Event Parsing & Layout", || -> Result<(), PipelineError> {
                let event_parser = EventParser::new(&self.stylesheet, &self.template_engine);
                let events = event_parser.parse(data)?;

                let mut layout_processor =
                    StreamingLayoutProcessor::new(&mut renderer, &self.stylesheet);

                for event in events {
                    layout_processor.process_event(event)?;
                }
                Ok(())
            });

        if let Err(e) = process_result {
            return Err(e);
        }

        let pdf_bytes = metrics
            .time_scope("PDF Finalization", || renderer.finalize_and_get_bytes(&self.template_engine))?;

        metrics.report();
        Ok(pdf_bytes)
    }

    pub fn generate_to_file<P: AsRef<std::path::Path>>(
        &self,
        data: &Value,
        path: P,
    ) -> Result<(), PipelineError> {
        let pdf_bytes = self.generate(data)?;
        std::fs::write(path, pdf_bytes)?;
        Ok(())
    }
}