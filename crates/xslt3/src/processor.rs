use crate::executor::TemplateExecutor3;
use crate::resolver::{StylesheetResolver, compile_stylesheet};
use petty_idf::IRNode;
use petty_style::stylesheet::Stylesheet;
use petty_template_core::{
    CompiledTemplate, DataSourceFormat, ExecutionConfig, TemplateError, TemplateExecutor,
    TemplateFeatures, TemplateFlags, TemplateMetadata, TemplateParser,
};
use petty_xslt::datasources::json::JsonVDocument;
use petty_xslt::datasources::xml::XmlDocument;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Xslt3Parser {
    resolver: Option<Arc<dyn StylesheetResolver>>,
}

impl Default for Xslt3Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Xslt3Parser {
    pub fn new() -> Self {
        Self { resolver: None }
    }

    pub fn with_resolver(resolver: Arc<dyn StylesheetResolver>) -> Self {
        Self {
            resolver: Some(resolver),
        }
    }
}

impl TemplateParser for Xslt3Parser {
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, TemplateError> {
        let mut compiled = compile_stylesheet(template_source)
            .map_err(|e| TemplateError::ParseError(e.to_string()))?;

        if let Some(ref resolver) = self.resolver {
            let base_uri = resource_base_path.to_string_lossy();
            compiled
                .resolve_imports_includes(|href| resolver.resolve(href, Some(&base_uri)))
                .map_err(|e| TemplateError::ParseError(e.to_string()))?;
            compiled.finalize_after_merge();
        }

        let main_template: Arc<dyn CompiledTemplate> = Arc::new(Xslt3Template {
            compiled: Arc::new(compiled),
            resource_base_path,
        });

        Ok(TemplateFeatures {
            main_template,
            role_templates: HashMap::new(),
        })
    }
}

struct Xslt3Template {
    compiled: Arc<crate::ast::CompiledStylesheet3>,
    resource_base_path: PathBuf,
}

impl TemplateExecutor for Xslt3Template {
    fn execute(
        &self,
        data_source: &str,
        config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, TemplateError> {
        match config.format {
            DataSourceFormat::Xml => {
                let doc = XmlDocument::parse(data_source)
                    .map_err(|e| TemplateError::ParseError(format!("XML parse error: {}", e)))?;
                let root_node = doc.root_node();
                let mut executor = TemplateExecutor3::new(&self.compiled, root_node, false)
                    .map_err(|e| TemplateError::ExecutionError(e.to_string()))?;
                executor
                    .build_tree()
                    .map_err(|e| TemplateError::ExecutionError(e.to_string()))
            }
            DataSourceFormat::Json => {
                let json_value: serde_json::Value = serde_json::from_str(data_source)
                    .map_err(|e| TemplateError::ParseError(format!("JSON parse error: {}", e)))?;
                let doc = JsonVDocument::new(&json_value);
                let root_node = doc.root_node();
                let mut executor = TemplateExecutor3::new(&self.compiled, root_node, false)
                    .map_err(|e| TemplateError::ExecutionError(e.to_string()))?;
                executor
                    .build_tree()
                    .map_err(|e| TemplateError::ExecutionError(e.to_string()))
            }
        }
    }
}

impl TemplateMetadata for Xslt3Template {
    fn stylesheet(&self) -> Arc<Stylesheet> {
        Arc::new(self.compiled.stylesheet.clone())
    }

    fn resource_base_path(&self) -> &Path {
        &self.resource_base_path
    }

    fn features(&self) -> TemplateFlags {
        let features = &self.compiled.features;
        TemplateFlags {
            has_table_of_contents: false,
            has_page_number_placeholders: false,
            uses_index_function: false,
            has_internal_links: features.uses_streaming || features.uses_accumulators,
        }
    }
}

pub fn detect_xslt_version(source: &str) -> XsltVersion {
    if source.contains(r#"version="3.0""#) || source.contains(r#"version='3.0'"#) {
        XsltVersion::V30
    } else if source.contains(r#"version="2.0""#) || source.contains(r#"version='2.0'"#) {
        XsltVersion::V20
    } else {
        XsltVersion::V10
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XsltVersion {
    V10,
    V20,
    V30,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_detection_v10() {
        let xslt =
            r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"/>"#;
        assert_eq!(detect_xslt_version(xslt), XsltVersion::V10);
    }

    #[test]
    fn test_version_detection_v30() {
        let xslt =
            r#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"/>"#;
        assert_eq!(detect_xslt_version(xslt), XsltVersion::V30);
    }

    #[test]
    fn test_xslt3_parser() {
        let xslt = r#"
            <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <p>Hello World</p>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let parser = Xslt3Parser::new();
        let result = parser.parse(xslt, PathBuf::new());
        assert!(result.is_ok());
    }
}
