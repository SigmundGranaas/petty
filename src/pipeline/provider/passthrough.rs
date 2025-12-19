use petty_core::error::PipelineError;
use crate::pipeline::api::PreparedDataSources;
use crate::pipeline::provider::DataSourceProvider;
use serde_json::Value;
use crate::pipeline::context::PipelineContext;

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
    use petty_core::core::layout::fonts::SharedFontLibrary;
    use petty_core::parser::json::processor::JsonParser;
    use petty_core::parser::processor::TemplateParser;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn pass_through_provider_works() {
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
            font_library: Arc::new(SharedFontLibrary::new()),
            resource_provider: Arc::new(crate::resource::InMemoryResourceProvider::new()),
            executor: crate::executor::ExecutorImpl::Sync(crate::executor::SyncExecutor::new()),
            cache_config: Default::default(),
        };

        let provider = PassThroughProvider;
        let data = vec![json!(1), json!(2), json!(3)];
        let iterator = data.into_iter();

        let prepared_sources = provider.provide(&context, iterator).unwrap();

        assert!(prepared_sources.document.is_none());
        assert!(prepared_sources.body_artifact.is_none());

        let collected_data: Vec<Value> = prepared_sources.data_iterator.collect();
        assert_eq!(collected_data, vec![json!(1), json!(2), json!(3)]);
    }
}