mod event;
mod handlers;
mod table;

use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::render::DocumentRenderer;
use crate::stylesheet::Stylesheet;
use handlebars::Handlebars;
use serde_json::Value;

// Re-export the Event enum to be the public face of this module
pub use event::Event;

/// Parses stylesheet templates and data, feeding `Event`s directly to a processor.
pub struct Parser<'a> {
    // These fields are `pub(super)` to be accessible by the handlers in submodules.
    pub(super) stylesheet: &'a Stylesheet,
    pub(super) template_engine: &'a Handlebars<'static>,
}

impl<'a> Parser<'a> {
    pub fn new(stylesheet: &'a Stylesheet, template_engine: &'a Handlebars<'static>) -> Self {
        Parser {
            stylesheet,
            template_engine,
        }
    }

    /// The main entry point for parsing. It iterates through page sequences and
    /// kicks off the recursive parsing for each logical document.
    pub fn parse<R: DocumentRenderer<'a>>(
        &self,
        data: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError> {
        processor.process_event(Event::StartDocument)?;

        if self.stylesheet.page_sequences.is_empty() {
            return Err(PipelineError::StylesheetError(
                "No page_sequences defined in stylesheet.".to_string(),
            ));
        }

        for (_seq_name, sequence) in &self.stylesheet.page_sequences {
            let template = self
                .stylesheet
                .templates
                .get(&sequence.template)
                .ok_or_else(|| {
                    PipelineError::TemplateParseError(format!(
                        "Template '{}' not found",
                        sequence.template
                    ))
                })?;

            let data_items: Vec<&Value> = if sequence.data_source == "/" {
                vec![data]
            } else {
                data.pointer(&sequence.data_source)
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().collect())
                    .ok_or_else(|| {
                        PipelineError::TemplateParseError(format!(
                            "Data source '{}' not found or not an array",
                            sequence.data_source
                        ))
                    })?
            };

            for item_data in data_items {
                processor.process_event(Event::BeginPageSequenceItem { context: item_data })?;
                // Delegate to the recursive handler
                self.parse_children(&template.children, item_data, processor)?;
                processor.process_event(Event::EndPageSequenceItem)?;
            }
        }

        processor.process_event(Event::EndDocument)?;
        Ok(())
    }
}