// src/pipeline/provider/passthrough.rs
use crate::error::PipelineError;
use crate::pipeline::api::PreparedDataSources;
use crate::pipeline::provider::DataSourceProvider;
use serde_json::Value;
use crate::pipeline::context::PipelineContext;

/// A simple provider that performs no analysis.
///
/// It acts as a zero-cost abstraction, taking the user's data iterator,
/// boxing it, and placing it into a `PreparedDataSources` struct for
/// a streaming renderer to consume.
#[derive(Clone)]
pub struct PassThroughProvider;

impl DataSourceProvider for PassThroughProvider {
    fn provide<'a, I>(
        &self,
        _context: &'a PipelineContext,
        data_iterator: I,
    ) -> Result<PreparedDataSources, PipelineError>
    where
        I: Iterator<Item = Value> + Send + 'static,
    {
        Ok(PreparedDataSources {
            data_iterator: Box::new(data_iterator),
            document: None,
            body_artifact: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::FontManager;
    use crate::parser::json::processor::JsonParser;
    use crate::parser::processor::TemplateParser;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn pass_through_provider_works() {
        // 1. Setup a mock context
        let parser = JsonParser;
        let template_str = r#"
        {
            "_stylesheet": {
                "pageMasters": {},
                "styles": {}
            },
            "_template": { "type": "Block", "children": [] }
        }
        "#;
        let features = parser.parse(template_str, PathBuf::new()).unwrap();
        let context = PipelineContext {
            compiled_template: features.main_template,
            role_templates: Arc::new(HashMap::new()),
            font_manager: Arc::new(FontManager::new()),
        };

        // 2. Create the provider and a sample iterator
        let provider = PassThroughProvider;
        let data = vec![json!(1), json!(2), json!(3)];
        let iterator = data.into_iter();

        // 3. Call provide
        let prepared_sources = provider.provide(&context, iterator).unwrap();

        // 4. Assertions
        assert!(prepared_sources.document.is_none());
        assert!(prepared_sources.body_artifact.is_none());

        let collected_data: Vec<Value> = prepared_sources.data_iterator.collect();
        assert_eq!(collected_data, vec![json!(1), json!(2), json!(3)]);
    }
}