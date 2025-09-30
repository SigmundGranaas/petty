//! Implements the "Compilation" phase for the JSON parser.
//! It transforms the Serde-parsed AST into a validated, executable instruction set.

use super::ast::{self, ControlNode, JsonNode, TemplateNode};
use crate::parser::ParseError;
use std::sync::Arc;
use itertools::Itertools;
use crate::core::idf::TableColumnDefinition;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
// --- Executable Instruction Set ---

/// A pre-compiled, executable instruction. This is the output of the `Compiler`.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonInstruction {
    // Block-level
    Block {
        styles: CompiledStyles,
        children: Vec<JsonInstruction>,
    },
    FlexContainer {
        styles: CompiledStyles,
        children: Vec<JsonInstruction>,
    },
    List {
        styles: CompiledStyles,
        children: Vec<JsonInstruction>,
    },
    ListItem {
        styles: CompiledStyles,
        children: Vec<JsonInstruction>,
    },
    Paragraph {
        styles: CompiledStyles,
        children: Vec<JsonInstruction>,
    },
    Image {
        styles: CompiledStyles,
        src_template: String,
    },
    Table(CompiledTable),

    // Inline-level
    Text {
        content_template: String,
    },
    StyledSpan {
        styles: CompiledStyles,
        children: Vec<JsonInstruction>,
    },
    Hyperlink {
        styles: CompiledStyles,
        href_template: String,
        children: Vec<JsonInstruction>,
    },
    InlineImage {
        styles: CompiledStyles,
        src_template: String,
    },
    LineBreak,

    // Control Flow
    ForEach {
        in_path: String,
        body: Vec<JsonInstruction>,
    },
    If {
        test: String,
        then_branch: Vec<JsonInstruction>,
        else_branch: Vec<JsonInstruction>,
    },
}

/// A pre-compiled representation of styles for an element.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompiledStyles {
    /// A list of pre-resolved, shared pointers to named styles.
    pub static_styles: Vec<Arc<ElementStyle>>,
    /// A list of template strings for style names, to be rendered at execution time.
    pub dynamic_style_templates: Vec<String>,
    /// An optional inline style override.
    pub style_override: Option<ElementStyle>,
}

/// A pre-compiled representation of a `<table>` node.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTable {
    pub styles: CompiledStyles,
    pub columns: Vec<TableColumnDefinition>,
    pub header: Option<Vec<JsonInstruction>>,
    pub body: Vec<JsonInstruction>,
}

// --- The Compiler ---

/// A stateful compiler that transforms a Serde-parsed JSON AST into an executable instruction set.
pub struct Compiler<'a> {
    stylesheet: &'a Stylesheet,
}

impl<'a> Compiler<'a> {
    /// Creates a new compiler with a reference to the stylesheet for style resolution.
    pub fn new(stylesheet: &'a Stylesheet) -> Self {
        Self { stylesheet }
    }

    /// The main entry point for compiling a `TemplateNode` AST.
    pub fn compile(&self, root_node: &TemplateNode) -> Result<Vec<JsonInstruction>, ParseError> {
        self.compile_node(root_node)
    }

    /// Recursively compiles a `TemplateNode` into a `Vec` of `JsonInstruction`s.
    /// A `Vec` is returned because control flow nodes can expand to multiple instructions.
    fn compile_node(&self, node: &TemplateNode) -> Result<Vec<JsonInstruction>, ParseError> {
        match node {
            TemplateNode::Static(static_node) => {
                Ok(vec![self.compile_static_node(static_node)?])
            }
            TemplateNode::Control(control_node) => self.compile_control_node(control_node),
        }
    }

    /// Compiles control flow nodes like `if` and `each`.
    fn compile_control_node(
        &self,
        node: &ControlNode,
    ) -> Result<Vec<JsonInstruction>, ParseError> {
        match node {
            ControlNode::Each { each, template } => {
                let compiled_body = self.compile_node(template)?;
                Ok(vec![JsonInstruction::ForEach {
                    in_path: each.clone(),
                    body: compiled_body,
                }])
            }
            ControlNode::If {
                test,
                then,
                else_branch,
            } => {
                let then_branch = self.compile_node(then)?;
                let else_branch = match else_branch {
                    Some(branch) => self.compile_node(branch)?,
                    None => Vec::new(),
                };
                Ok(vec![JsonInstruction::If {
                    test: test.clone(),
                    then_branch,
                    else_branch,
                }])
            }
        }
    }

    /// Compiles a static (non-control-flow) node.
    fn compile_static_node(&self, node: &JsonNode) -> Result<JsonInstruction, ParseError> {
        match node {
            // Block-level
            JsonNode::Block(c) => Ok(JsonInstruction::Block {
                styles: self.compile_styles(&c.style_names, &c.style_override)?,
                children: self.compile_children(&c.children)?,
            }),
            JsonNode::FlexContainer(c) => Ok(JsonInstruction::FlexContainer {
                styles: self.compile_styles(&c.style_names, &c.style_override)?,
                children: self.compile_children(&c.children)?,
            }),
            JsonNode::List(c) => Ok(JsonInstruction::List {
                styles: self.compile_styles(&c.style_names, &c.style_override)?,
                children: self.compile_children(&c.children)?,
            }),
            JsonNode::ListItem(c) => Ok(JsonInstruction::ListItem {
                styles: self.compile_styles(&c.style_names, &c.style_override)?,
                children: self.compile_children(&c.children)?,
            }),
            JsonNode::Paragraph(p) => Ok(JsonInstruction::Paragraph {
                styles: self.compile_styles(&p.style_names, &p.style_override)?,
                children: self.compile_children(&p.children)?,
            }),
            JsonNode::Image(i) => Ok(JsonInstruction::Image {
                styles: self.compile_styles(&i.style_names, &i.style_override)?,
                src_template: i.src.clone(),
            }),
            JsonNode::Table(t) => self.compile_table_node(t),

            // Inline-level
            JsonNode::Text { content } => Ok(JsonInstruction::Text {
                content_template: content.clone(),
            }),
            JsonNode::StyledSpan(c) => Ok(JsonInstruction::StyledSpan {
                styles: self.compile_styles(&c.style_names, &c.style_override)?,
                children: self.compile_children(&c.children)?,
            }),
            JsonNode::Hyperlink(h) => Ok(JsonInstruction::Hyperlink {
                styles: self.compile_styles(&h.style_names, &h.style_override)?,
                href_template: h.href.clone(),
                children: self.compile_children(&h.children)?,
            }),
            JsonNode::InlineImage(i) => Ok(JsonInstruction::InlineImage {
                styles: self.compile_styles(&i.style_names, &i.style_override)?,
                src_template: i.src.clone(),
            }),
            JsonNode::LineBreak => Ok(JsonInstruction::LineBreak),
        }
    }

    /// Compiles the complex `Table` node.
    fn compile_table_node(&self, table: &ast::JsonTable) -> Result<JsonInstruction, ParseError> {
        let header = match &table.header {
            Some(h) => Some(self.compile_children(&h.rows)?),
            None => None,
        };

        Ok(JsonInstruction::Table(CompiledTable {
            styles: self.compile_styles(&table.style_names, &table.style_override)?,
            columns: table
                .columns
                .iter()
                .map(|c| TableColumnDefinition {
                    width: c.width.clone(),
                    style: c.style.clone(),
                    header_style: c.header_style.clone(),
                })
                .collect(),
            header,
            body: self.compile_children(&table.body.rows)?,
        }))
    }

    /// Helper to compile a `Vec` of `TemplateNode` children.
    fn compile_children(
        &self,
        children: &[TemplateNode],
    ) -> Result<Vec<JsonInstruction>, ParseError> {
        children
            .iter()
            .map(|node| self.compile_node(node))
            .flatten_ok()
            .collect()
    }

    /// Compiles style information, resolving static names and preserving dynamic templates.
    fn compile_styles(
        &self,
        names: &[String],
        style_override: &ElementStyle,
    ) -> Result<CompiledStyles, ParseError> {
        let mut static_styles = Vec::new();
        let mut dynamic_style_templates = Vec::new();

        for name_str in names {
            if name_str.contains("{{") {
                dynamic_style_templates.push(name_str.clone());
            } else {
                for name in name_str.split_whitespace() {
                    if !name.is_empty() {
                        let style = self
                            .stylesheet
                            .styles
                            .get(name)
                            .cloned()
                            .ok_or_else(|| {
                                ParseError::TemplateParse(format!(
                                    "Style '{}' not found in stylesheet",
                                    name
                                ))
                            })?;
                        static_styles.push(style);
                    }
                }
            }
        }

        Ok(CompiledStyles {
            static_styles,
            dynamic_style_templates,
            style_override: if *style_override == ElementStyle::default() {
                None
            } else {
                Some(style_override.clone())
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::json::ast::JsonParagraph;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_compiler() -> (Compiler<'static>, Stylesheet) {
        let mut styles = HashMap::new();
        styles.insert(
            "test_style".to_string(),
            Arc::new(ElementStyle {
                font_size: Some(12.0),
                ..Default::default()
            }),
        );
        let mut stylesheet = Stylesheet::default();
        stylesheet.styles = styles;
        // We need to leak the stylesheet to get a 'static lifetime for the test
        let static_stylesheet: &'static Stylesheet = Box::leak(Box::new(stylesheet));
        (Compiler::new(static_stylesheet), static_stylesheet.clone())
    }

    #[test]
    fn test_compile_static_node() {
        let (compiler, _) = create_test_compiler();
        let node = TemplateNode::Static(JsonNode::Paragraph(JsonParagraph {
            style_names: vec!["test_style".to_string()],
            children: vec![TemplateNode::Static(JsonNode::Text {
                content: "Hello".to_string(),
            })],
            ..Default::default()
        }));

        let result = compiler.compile(&node).unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            JsonInstruction::Paragraph { styles, children } => {
                assert_eq!(styles.static_styles.len(), 1);
                assert_eq!(styles.static_styles[0].font_size, Some(12.0));
                assert!(styles.dynamic_style_templates.is_empty());
                assert_eq!(children.len(), 1);
                assert!(matches!(&children[0], JsonInstruction::Text { .. }));
            }
            _ => panic!("Expected Paragraph instruction"),
        }
    }

    #[test]
    fn test_compile_fails_on_missing_style() {
        let (compiler, _) = create_test_compiler();
        let node = TemplateNode::Static(JsonNode::Paragraph(JsonParagraph {
            style_names: vec!["non_existent_style".to_string()],
            ..Default::default()
        }));

        let result = compiler.compile(&node);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Style 'non_existent_style' not found"));
    }

    #[test]
    fn test_compile_if_then_else() {
        let (compiler, _) = create_test_compiler();
        let node: TemplateNode = serde_json::from_value(json!({
            "if": "{{ show_it }}",
            "then": { "type": "Text", "content": "Then" },
            "else": { "type": "Text", "content": "Else" }
        }))
            .unwrap();

        let result = compiler.compile(&node).unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            JsonInstruction::If {
                test,
                then_branch,
                else_branch,
            } => {
                assert_eq!(test, "{{ show_it }}");
                assert_eq!(then_branch.len(), 1);
                assert_eq!(else_branch.len(), 1);
                assert!(matches!(&then_branch[0], JsonInstruction::Text { .. }));
                assert!(matches!(&else_branch[0], JsonInstruction::Text { .. }));
            }
            _ => panic!("Expected If instruction"),
        }
    }

    #[test]
    fn test_compile_foreach() {
        let (compiler, _) = create_test_compiler();
        let node: TemplateNode = serde_json::from_value(json!({
            "each": "items",
            "template": {
                "type": "Paragraph",
                "children": [{ "type": "Text", "content": "{{ this.name }}" }]
            }
        }))
            .unwrap();

        let result = compiler.compile(&node).unwrap();
        assert_eq!(result.len(), 1);

        match &result[0] {
            JsonInstruction::ForEach { in_path, body } => {
                assert_eq!(in_path, "items");
                assert_eq!(body.len(), 1);
                assert!(matches!(&body[0], JsonInstruction::Paragraph { .. }));
            }
            _ => panic!("Expected ForEach instruction"),
        }
    }
}