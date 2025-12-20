//! Integration adapters for template parsers
//!
//! This module provides adapters that allow parsers implementing
//! petty_template_core traits to work with petty_core traits.

use petty_core::error::PipelineError;
use petty_core::parser::processor::{TemplateFeatures, TemplateParser};
use std::path::PathBuf;

/// Adapter that wraps a petty_template_core::TemplateParser to implement
/// petty_core::parser::processor::TemplateParser
pub struct TemplateParserAdapter<P> {
    inner: P,
}

impl<P> TemplateParserAdapter<P> {
    pub fn new(parser: P) -> Self {
        Self { inner: parser }
    }
}

impl<P> TemplateParser for TemplateParserAdapter<P>
where
    P: petty_template_core::TemplateParser,
{
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, PipelineError> {
        Ok(TemplateFeatures::from_core(
            self.inner.parse(template_source, resource_base_path)?,
        ))
    }
}
