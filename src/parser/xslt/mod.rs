mod nodes;
mod tags;
mod util;

use super::processor::TemplateProcessor;
use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::parser::Event;
use crate::render::DocumentRenderer;
use crate::stylesheet::Stylesheet;
use handlebars::Handlebars;
use log;
use quick_xml::Reader;
use serde_json::Value;

/// Processes XML-based templates with an XSLT-like syntax.
pub struct XsltTemplateParser<'a> {
    xslt_content: &'a str,
    stylesheet: &'a Stylesheet,
    template_engine: Handlebars<'static>,
    // State for tracking table context during parsing.
    pub(super) row_column_index_stack: Vec<usize>,
}

impl<'a> XsltTemplateParser<'a> {
    pub fn new(
        xslt_content: &'a str,
        stylesheet: &'a Stylesheet,
        template_engine: Handlebars<'static>,
    ) -> Self {
        Self {
            xslt_content,
            stylesheet,
            template_engine,
            row_column_index_stack: Vec::new(),
        }
    }
}

impl<'a> TemplateProcessor<'a> for XsltTemplateParser<'a> {
    fn process<R: DocumentRenderer<'a>>(
        &mut self,
        data: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError> {
        log::info!("Starting XSLT template processing...");
        let mut reader = Reader::from_str(self.xslt_content);
        reader.config_mut().trim_text(true);

        processor.process_event(Event::StartDocument)?;

        // The XSLT template itself is now responsible for creating page sequences.
        // The parser just processes nodes from the root of the template.
        nodes::parse_nodes(self, &mut reader, data, processor)?;

        processor.process_event(Event::EndDocument)?;

        log::info!("XSLT template processing finished.");
        Ok(())
    }
}