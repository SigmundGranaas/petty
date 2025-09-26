//! The main public entry point for the JSON parser module.
//! It orchestrates the compile and execute phases.

use super::ast::JsonTemplateFile;
use super::compiler::Compiler;
use super::executor::TemplateExecutor;
use crate::parser::ParseError;
use handlebars::Handlebars;
use serde_json::Value;
use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;

/// Processes a JSON template against a data context to produce an `IRNode` tree.
pub struct JsonProcessor<'h> {
    template_content: &'h str,
    handlebars: &'h Handlebars<'static>,
}

impl<'h> JsonProcessor<'h> {
    pub fn new(template_content: &'h str, handlebars: &'h Handlebars<'static>) -> Self {
        Self {
            template_content,
            handlebars,
        }
    }

    /// Builds the `IRNode` tree by running the full compile-execute pipeline.
    pub fn build_tree(&self, context: &Value) -> Result<Vec<IRNode>, ParseError> {
        // --- Phase 1: Parse & Compile ---

        // Use Serde to parse the raw JSON string into our initial AST.
        let template_file: JsonTemplateFile = serde_json::from_str(self.template_content)?;
        let stylesheet = Stylesheet::from(template_file._stylesheet);

        // Compile the Serde AST into a validated, executable instruction set.
        // This is where style names are resolved and the template structure is verified.
        let compiler = Compiler::new(&stylesheet);
        let instructions = compiler.compile(&template_file._template)?;

        // --- Phase 2: Execute ---

        // Execute the compiled instructions against the provided data context.
        // This is where Handlebars expressions are rendered and control flow is executed.
        let mut executor = TemplateExecutor::new(self.handlebars, &stylesheet);
        executor.build_tree(&instructions, context)
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
            "styles": {
              "title": { "fontSize": 18.0, "fontWeight": "bold" },
              "item_para": { "margin": "4pt" }
            }
          },
          "_template": {
            "type": "Block",
            "children": [
              {
                "type": "Paragraph",
                "styleNames": ["title"],
                "children": [{ "type": "Text", "content": "Report for {{ customer.name }}" }]
              },
              {
                "if": "{{ customer.is_premium }}",
                "then": {
                  "type": "Paragraph",
                  "children": [{ "type": "Text", "content": "Premium Member" }]
                }
              },
              {
                "each": "products",
                "template": {
                  "type": "Paragraph",
                  "styleNames": ["item_para"],
                  "children": [{ "type": "Text", "content": "- {{ this.name }}: ${{ this.price }}" }]
                }
              }
            ]
          }
        }
        "#
    }

    #[test]
    fn test_full_pipeline_premium_customer() {
        let handlebars = Handlebars::new();
        let processor = JsonProcessor::new(get_test_template(), &handlebars);
        let data = json!({
            "customer": {
                "name": "Acme Inc.",
                "is_premium": true
            },
            "products": [
                { "name": "Anvil", "price": 100 },
                { "name": "Rocket", "price": 5000 }
            ]
        });

        let tree = processor.build_tree(&data).unwrap();
        assert_eq!(tree.len(), 1); // Root block

        let root_children = match &tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        assert_eq!(root_children.len(), 4); // title, if-then, 2x each-body

        // Check title
        assert!(matches!(&root_children[0], IRNode::Paragraph { .. }));
        // Check premium status
        assert!(matches!(&root_children[1], IRNode::Paragraph { .. }));
        // Check items
        assert!(matches!(&root_children[2], IRNode::Paragraph { .. }));
        assert!(matches!(&root_children[3], IRNode::Paragraph { .. }));
    }

    #[test]
    fn test_full_pipeline_non_premium_customer() {
        let handlebars = Handlebars::new();
        let processor = JsonProcessor::new(get_test_template(), &handlebars);
        let data = json!({
            "customer": {
                "name": "Contoso",
                "is_premium": false
            },
            "products": []
        });

        let tree = processor.build_tree(&data).unwrap();
        let root_children = match &tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };

        // Only the title paragraph should be rendered. The 'if' is false, and 'each' is empty.
        assert_eq!(root_children.len(), 1);
    }
}