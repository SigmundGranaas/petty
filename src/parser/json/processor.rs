// FILE: /home/sigmund/RustroverProjects/petty/src/parser/json/processor.rs
//! Implements the public interface for the JSON parser, conforming to the
//! `TemplateParser` and `CompiledTemplate` traits.

use super::ast::JsonTemplateFile;
use super::compiler::{Compiler, JsonInstruction};
use super::executor::TemplateExecutor;
use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use crate::parser::processor::{CompiledTemplate, TemplateParser};
use crate::parser::ParseError;
use handlebars::Handlebars;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// --- The Compiled Artifact ---

#[derive(Debug)]
pub struct CompiledJsonTemplate {
    instructions: Vec<JsonInstruction>,
    definitions: HashMap<String, Vec<JsonInstruction>>,
    stylesheet: Stylesheet,
    resource_base_path: PathBuf,
    handlebars: Handlebars<'static>,
}

impl CompiledTemplate for CompiledJsonTemplate {
    fn execute(&self, data: &Value) -> Result<Vec<IRNode>, PipelineError> {
        let mut executor = TemplateExecutor::new(&self.handlebars, &self.stylesheet, &self.definitions);
        let ir_nodes = executor.build_tree(&self.instructions, data)?;
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

pub struct JsonParser;

impl TemplateParser for JsonParser {
    fn parse(&self, template_source: &str, resource_base_path: PathBuf) -> Result<Arc<dyn CompiledTemplate>, ParseError> {
        // Phase 1: Deserialize the raw JSON string into our AST.
        let template_file: JsonTemplateFile = serde_json::from_str(template_source)?;
        let mut stylesheet = Stylesheet::from(template_file._stylesheet.clone());

        // If a default master isn't set, pick the first one found.
        if stylesheet.default_page_master_name.is_none() {
            stylesheet.default_page_master_name = stylesheet.page_masters.keys().next().cloned();
        }

        // Phase 2: Pre-compile all template definitions (partials).
        let empty_defs = HashMap::new(); // Cannot refer to other defs
        let def_compiler = Compiler::new(&stylesheet, &empty_defs);

        let compiled_definitions: HashMap<String, Vec<JsonInstruction>> = template_file
            ._stylesheet
            .definitions
            .iter()
            .map(|(name, node)| def_compiler.compile(node).map(|instr| (name.clone(), instr)))
            .collect::<Result<_, _>>()?;

        // Phase 3: Compile the main template body, providing the compiled definitions for validation.
        let main_compiler = Compiler::new(&stylesheet, &compiled_definitions);
        let main_instructions = main_compiler.compile(&template_file._template)?;

        // Phase 4: Construct the final compiled artifact.
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(false);

        let compiled_template = CompiledJsonTemplate {
            instructions: main_instructions,
            definitions: compiled_definitions,
            stylesheet,
            resource_base_path,
            handlebars,
        };

        Ok(Arc::new(compiled_template))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn get_test_template() -> &'static str {
        r#"
        {
          "_stylesheet": {
            "pageMasters": {
              "main": { "size": "A4" }
            },
            "styles": {
              "title": { "fontSize": 18.0, "fontWeight": "bold" },
              "item_para": { "margin": "4pt" }
            },
            "definitions": {}
          },
          "_template": {
            "type": "Block",
            "children": [
              { "type": "Paragraph", "styleNames": ["title"], "children": [{ "type": "Text", "content": "Report for {{ customer.name }}" }] },
              { "if": "{{ customer.is_premium }}", "then": { "type": "Paragraph", "children": [{ "type": "Text", "content": "Premium Member" }] } },
              { "each": "products", "template": { "type": "Paragraph", "styleNames": ["item_para"], "children": [{ "type": "Text", "content": "- {{ this.name }}: ${{ this.price }}" }] } }
            ]
          }
        }
        "#
    }

    #[test]
    fn test_full_pipeline_premium_customer() {
        let parser = JsonParser;
        let compiled_template = parser.parse(get_test_template(), PathBuf::new()).unwrap();
        let data = json!({
            "customer": { "name": "Acme Inc.", "is_premium": true },
            "products": [ { "name": "Anvil", "price": 100 }, { "name": "Rocket", "price": 5000 } ]
        });

        let tree = compiled_template.execute(&data).unwrap();
        // The root is not part of the children count from build_tree
        assert_eq!(tree.len(), 1);
        let root_children = match &tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!(),
        };
        // Title para, premium para, 2x product para = 4 children
        assert_eq!(root_children.len(), 4);
    }

    #[test]
    fn test_full_pipeline_non_premium_customer() {
        let parser = JsonParser;
        let compiled_template = parser.parse(get_test_template(), PathBuf::new()).unwrap();
        let data = json!({ "customer": { "name": "Contoso", "is_premium": false }, "products": [] });
        let tree = compiled_template.execute(&data).unwrap();
        let root_children = match &tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!(),
        };
        // Just the title para
        assert_eq!(root_children.len(), 1);
    }
}