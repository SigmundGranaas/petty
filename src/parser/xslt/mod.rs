// src/parser/xslt/mod.rs
mod nodes;
mod tags;
mod util;

use super::processor::{LayoutProcessorProxy, TemplateProcessor};
use crate::error::PipelineError;
use crate::idf::IDFEvent;
use crate::stylesheet::Stylesheet;
use async_trait::async_trait;
use handlebars::Handlebars;
use log;
use quick_xml::events::Event as XmlEvent;
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

#[async_trait(?Send)]
impl<'a> TemplateProcessor<'a> for XsltTemplateParser<'a> {
    async fn process(
        &mut self,
        data: &'a Value,
        proxy: &mut LayoutProcessorProxy<'a>,
    ) -> Result<(), PipelineError> {
        log::info!("Starting XSLT template processing...");
        let mut reader = Reader::from_str(self.xslt_content);
        reader.config_mut().trim_text(true);

        proxy.process_event(IDFEvent::StartDocument).await?;

        let mut buf = Vec::new();
        let mut found_root_template = false;

        // Walk the XML event stream to find the root template.
        // We don't need to manually manage hierarchy, just look for the specific start event.
        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:template" => {
                    let is_root = e.attributes().flatten().any(|attr| {
                        if attr.key.as_ref() == b"match" {
                            // Correctly handle the Result before comparing the value.
                            matches!(attr.unescape_value(), Ok(value) if value == "/")
                        } else {
                            false
                        }
                    });

                    if is_root {
                        log::debug!("Found root template <xsl:template match=\"/\">. Processing content...");
                        // This is our entry point. Start parsing the children of this node.
                        nodes::parse_nodes(self, &mut reader, data, proxy).await?;
                        found_root_template = true;
                        break; // Document content is generated, we can stop.
                    }
                }
                XmlEvent::Eof => break, // Reached end of file without finding the root template.
                _ => (),             // Ignore all other events (other tags, text, comments etc.)
            }
            buf.clear();
        }

        if !found_root_template {
            return Err(PipelineError::TemplateParseError(
                "Could not find a root <xsl:template match=\"/\"> in the XSLT file.".to_string(),
            ));
        }

        proxy.process_event(IDFEvent::EndDocument).await?;

        log::info!("XSLT template processing finished.");
        Ok(())
    }
}