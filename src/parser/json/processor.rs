use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use crate::parser::processor::{CompiledTemplate, ExecutionConfig, TemplateParser};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{ast, compiler, executor};

#[derive(Debug)]
pub struct JsonTemplate {
    instructions: Vec<compiler::JsonInstruction>,
    definitions: HashMap<String, Vec<compiler::JsonInstruction>>,
    stylesheet: Arc<Stylesheet>,
    resource_base_path: PathBuf,
}

impl CompiledTemplate for JsonTemplate {
    fn execute(
        &self,
        data_source: &str,
        _config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, PipelineError> {
        let context: serde_json::Value =
            serde_json::from_str(data_source).map_err(|e| PipelineError::Parse(e.into()))?;
        let mut executor = executor::TemplateExecutor::new(&self.stylesheet, &self.definitions);
        let ir_tree = executor
            .build_tree(&self.instructions, &context)
            .map_err(PipelineError::Parse)?;
        Ok(ir_tree)
    }

    fn stylesheet(&self) -> Arc<Stylesheet> {
        Arc::clone(&self.stylesheet)
    }

    fn resource_base_path(&self) -> &Path {
        &self.resource_base_path
    }
}

pub struct JsonParser;

impl TemplateParser for JsonParser {
    fn parse(
        &self,
        source: &str,
        resource_base_path: PathBuf,
    ) -> Result<Arc<dyn CompiledTemplate>, PipelineError> {
        let file_ast: ast::JsonTemplateFile = serde_json::from_str(source)?;

        let stylesheet = Stylesheet {
            page_masters: file_ast._stylesheet.page_masters,
            styles: file_ast
                ._stylesheet
                .styles
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            ..Default::default()
        };
        let definitions_ast = file_ast._stylesheet.definitions;

        // The compiler for definitions needs an empty set of definitions to avoid recursion issues.
        let empty_defs = HashMap::new();
        let def_compiler = compiler::Compiler::new(&stylesheet, &empty_defs);
        let compiled_definitions: HashMap<String, Vec<compiler::JsonInstruction>> = definitions_ast
            .into_iter()
            .map(|(name, node)| def_compiler.compile(&node).map(|instr| (name, instr)))
            .collect::<Result<_, _>>()?;

        let compiler = compiler::Compiler::new(&stylesheet, &compiled_definitions);
        let instructions = compiler.compile(&file_ast._template)?;

        Ok(Arc::new(JsonTemplate {
            instructions,
            definitions: compiled_definitions,
            stylesheet: Arc::new(stylesheet),
            resource_base_path,
        }))
    }
}