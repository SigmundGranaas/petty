//! Implements the public interface for the XSLT parser, conforming to the
//! `TemplateParser` and `CompiledTemplate` traits.

use super::ast::CompiledStylesheet;
use super::compiler;
use super::executor;
use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use crate::parser::processor::{CompiledTemplate, TemplateParser};
use crate::parser::ParseError;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// --- The Compiled Artifact ---

impl CompiledTemplate for CompiledStylesheet {
    fn execute(&self, data: &Value) -> Result<Vec<IRNode>, PipelineError> {
        let mut executor = executor::TemplateExecutor::new(self);
        let ir_nodes = executor.build_tree(data)?;
        Ok(ir_nodes)
    }

    fn stylesheet(&self) -> &Stylesheet {
        &self.stylesheet
    }

    fn resource_base_path(&self) -> &Path {
        &self.resource_base_path
    }
}

// --- The Parser ---

pub struct XsltParser;

impl TemplateParser for XsltParser {
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<Arc<dyn CompiledTemplate>, ParseError> {
        // The compiler does all the work of parsing the stylesheet and template rules.
        let compiled_stylesheet = compiler::Compiler::compile(template_source, resource_base_path)?;
        Ok(Arc::new(compiled_stylesheet))
    }
}