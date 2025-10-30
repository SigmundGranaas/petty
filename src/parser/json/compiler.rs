//! Implements the "Compilation" phase for the JSON parser.
//! It transforms the Serde-parsed AST into a validated, executable instruction set.

use super::ast::{self, ControlNode, JsonNode, TemplateNode};
use crate::core::idf::TableColumnDefinition;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::parser::json::jpath::{self, Expression};
use crate::parser::ParseError;
use itertools::Itertools;
use std::collections::HashMap;
use std::sync::Arc;

// --- Custom Expression Parser (Replaces Handlebars for JSON) ---

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionPart {
    Static(String),
    /// A compiled JPath expression.
    Dynamic(Expression),
}

/// A pre-compiled string that is either static or a series of parts.
#[derive(Debug, Clone, PartialEq)]
pub enum CompiledString {
    Static(String),
    Dynamic(Vec<ExpressionPart>),
}

/// Parses a template string like "Hello {{ upper(user.name) }}" into parts.
pub fn parse_expression_string(text: &str) -> Result<CompiledString, ParseError> {
    if !text.contains("{{") {
        return Ok(CompiledString::Static(text.to_string()));
    }

    let mut parts = Vec::new();
    let mut last_end = 0;
    for (start, _part) in text.match_indices("{{") {
        if start > last_end {
            parts.push(ExpressionPart::Static(text[last_end..start].to_string()));
        }
        let end_marker = "}}";
        let end = text[start..]
            .find(end_marker)
            .ok_or_else(|| ParseError::TemplateParse("Unclosed {{ expression".to_string()))?;
        let inner = text[start + 2..start + end].trim();

        // Handle `this.` prefix for loop contexts to maintain compatibility.
        let path = inner.strip_prefix("this.").unwrap_or(inner);

        // Use the new, powerful expression parser
        let expression = jpath::parse_expression(path)?;
        parts.push(ExpressionPart::Dynamic(expression));
        last_end = start + end + 2;
    }
    if last_end < text.len() {
        parts.push(ExpressionPart::Static(text[last_end..].to_string()));
    }

    Ok(CompiledString::Dynamic(parts))
}

// --- Executable Instruction Set ---

/// A pre-compiled, executable instruction. This is the output of the `Compiler`.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonInstruction {
    Block { styles: CompiledStyles, children: Vec<JsonInstruction> },
    FlexContainer { styles: CompiledStyles, children: Vec<JsonInstruction> },
    List { styles: CompiledStyles, children: Vec<JsonInstruction> },
    ListItem { styles: CompiledStyles, children: Vec<JsonInstruction> },
    Paragraph { styles: CompiledStyles, children: Vec<JsonInstruction> },
    Image { styles: CompiledStyles, src: CompiledString },
    Table(CompiledTable),
    Text { content: CompiledString },
    StyledSpan { styles: CompiledStyles, children: Vec<JsonInstruction> },
    Hyperlink { styles: CompiledStyles, href: CompiledString, children: Vec<JsonInstruction> },
    InlineImage { styles: CompiledStyles, src: CompiledString },
    LineBreak,
    PageBreak { master_name: Option<String> },
    RenderTemplate { name: String },
    ForEach { select: Expression, body: Vec<JsonInstruction> },
    If { test: Expression, then_branch: Vec<JsonInstruction>, else_branch: Vec<JsonInstruction> },
    Heading { level: u8, styles: CompiledStyles, children: Vec<JsonInstruction> },
    TableOfContents { styles: CompiledStyles },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompiledStyles {
    pub id: Option<String>,
    pub static_styles: Vec<Arc<ElementStyle>>,
    pub dynamic_style_templates: Vec<CompiledString>,
    pub style_override: Option<ElementStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTable {
    pub styles: CompiledStyles,
    pub columns: Vec<TableColumnDefinition>,
    pub header: Option<Vec<JsonInstruction>>,
    pub body: Vec<JsonInstruction>,
}

/// A stateful compiler that transforms a Serde-parsed JSON AST into an executable instruction set.
pub struct Compiler<'a> {
    stylesheet: &'a Stylesheet,
    definitions: &'a HashMap<String, Vec<JsonInstruction>>,
}

impl<'a> Compiler<'a> {
    pub fn new(
        stylesheet: &'a Stylesheet,
        definitions: &'a HashMap<String, Vec<JsonInstruction>>,
    ) -> Self {
        Self { stylesheet, definitions }
    }
    pub fn compile(&self, root_node: &TemplateNode) -> Result<Vec<JsonInstruction>, ParseError> {
        self.compile_node(root_node)
    }

    fn compile_node(&self, node: &TemplateNode) -> Result<Vec<JsonInstruction>, ParseError> {
        match node {
            TemplateNode::Static(static_node) => Ok(vec![self.compile_static_node(static_node)?]),
            TemplateNode::Control(control_node) => self.compile_control_node(control_node),
        }
    }

    fn compile_control_node(&self, node: &ControlNode) -> Result<Vec<JsonInstruction>, ParseError> {
        match node {
            ControlNode::Each { each, template } => {
                let select = jpath::parse_expression(each)?;
                Ok(vec![JsonInstruction::ForEach { select, body: self.compile_node(template)? }])
            }
            ControlNode::If { test, then, else_branch } => {
                let test_expression = jpath::parse_expression(test)?;
                Ok(vec![JsonInstruction::If {
                    test: test_expression,
                    then_branch: self.compile_node(then)?,
                    else_branch: else_branch
                        .as_ref()
                        .map(|b| self.compile_node(b))
                        .transpose()?
                        .unwrap_or_default(),
                }])
            }
        }
    }

    fn compile_static_node(&self, node: &JsonNode) -> Result<JsonInstruction, ParseError> {
        match node {
            JsonNode::Block(c) => Ok(JsonInstruction::Block { styles: self.compile_styles(&c.style_names, &c.style_override, c.id.clone())?, children: self.compile_children(&c.children)? }),
            JsonNode::FlexContainer(c) => Ok(JsonInstruction::FlexContainer { styles: self.compile_styles(&c.style_names, &c.style_override, c.id.clone())?, children: self.compile_children(&c.children)? }),
            JsonNode::List(c) => Ok(JsonInstruction::List { styles: self.compile_styles(&c.style_names, &c.style_override, c.id.clone())?, children: self.compile_children(&c.children)? }),
            JsonNode::ListItem(c) => Ok(JsonInstruction::ListItem { styles: self.compile_styles(&c.style_names, &c.style_override, c.id.clone())?, children: self.compile_children(&c.children)? }),
            JsonNode::Paragraph(p) => Ok(JsonInstruction::Paragraph { styles: self.compile_styles(&p.style_names, &p.style_override, p.id.clone())?, children: self.compile_children(&p.children)? }),
            JsonNode::Image(i) => Ok(JsonInstruction::Image { styles: self.compile_styles(&i.style_names, &i.style_override, i.id.clone())?, src: parse_expression_string(&i.src)? }),
            JsonNode::Table(t) => self.compile_table_node(t),
            JsonNode::Heading(h) => Ok(JsonInstruction::Heading { level: h.level, styles: self.compile_styles(&h.style_names, &h.style_override, h.id.clone())?, children: self.compile_children(&h.children)? }),
            JsonNode::TableOfContents(c) => Ok(JsonInstruction::TableOfContents { styles: self.compile_styles(&c.style_names, &c.style_override, c.id.clone())? }),
            JsonNode::Text { content } => Ok(JsonInstruction::Text { content: parse_expression_string(content)? }),
            JsonNode::StyledSpan(c) => Ok(JsonInstruction::StyledSpan { styles: self.compile_styles(&c.style_names, &c.style_override, c.id.clone())?, children: self.compile_children(&c.children)? }),
            JsonNode::Hyperlink(h) => Ok(JsonInstruction::Hyperlink { styles: self.compile_styles(&h.style_names, &h.style_override, h.id.clone())?, href: parse_expression_string(&h.href)?, children: self.compile_children(&h.children)? }),
            JsonNode::InlineImage(i) => Ok(JsonInstruction::InlineImage { styles: self.compile_styles(&i.style_names, &i.style_override, i.id.clone())?, src: parse_expression_string(&i.src)? }),
            JsonNode::LineBreak => Ok(JsonInstruction::LineBreak),
            JsonNode::PageBreak { master_name } => Ok(JsonInstruction::PageBreak { master_name: master_name.clone() }),
            JsonNode::RenderTemplate { name } => {
                if !self.definitions.contains_key(name) {
                    return Err(ParseError::TemplateParse(format!("Defined template '{}' not found in stylesheet definitions.", name)));
                }
                Ok(JsonInstruction::RenderTemplate { name: name.clone() })
            }
        }
    }

    fn compile_table_node(&self, table: &ast::JsonTable) -> Result<JsonInstruction, ParseError> {
        Ok(JsonInstruction::Table(CompiledTable {
            styles: self.compile_styles(&table.style_names, &table.style_override, table.id.clone())?,
            columns: table.columns.iter().map(|c| TableColumnDefinition { width: c.width.clone(), style: c.style.clone(), header_style: c.header_style.clone() }).collect(),
            header: table.header.as_ref().map(|h| self.compile_children(&h.rows)).transpose()?,
            body: self.compile_children(&table.body.rows)?,
        }))
    }

    fn compile_children(
        &self,
        children: &[TemplateNode],
    ) -> Result<Vec<JsonInstruction>, ParseError> {
        children.iter().map(|node| self.compile_node(node)).flatten_ok().collect()
    }

    fn compile_styles(
        &self,
        names: &[String],
        style_override: &ElementStyle,
        id: Option<String>,
    ) -> Result<CompiledStyles, ParseError> {
        let mut static_styles = Vec::new();
        let mut dynamic_style_templates = Vec::new();
        for name_str in names {
            if name_str.contains("{{") {
                dynamic_style_templates.push(parse_expression_string(name_str)?);
            } else {
                for name in name_str.split_whitespace().filter(|s| !s.is_empty()) {
                    let style = self.stylesheet.styles.get(name).cloned().ok_or_else(|| {
                        ParseError::TemplateParse(format!("Style '{}' not found in stylesheet", name))
                    })?;
                    static_styles.push(style);
                }
            }
        }
        Ok(CompiledStyles {
            id,
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
    use crate::core::style::stylesheet::ElementStyle;
    use crate::parser::json::jpath::ast::Selection;
    use crate::parser::json::ast::JsonParagraph;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_compiler() -> (Compiler<'static>, Stylesheet) {
        let mut styles = HashMap::new();
        styles.insert(
            "test_style".to_string(),
            Arc::new(ElementStyle { font_size: Some(12.0), ..Default::default() }),
        );
        let mut stylesheet = Stylesheet::default();
        stylesheet.styles = styles;
        let static_stylesheet: &'static Stylesheet = Box::leak(Box::new(stylesheet));
        let empty_defs = Box::leak(Box::new(HashMap::new()));
        (Compiler::new(static_stylesheet, empty_defs), static_stylesheet.clone())
    }

    #[test]
    fn test_compile_static_node() {
        let (compiler, _) = create_test_compiler();
        let node = TemplateNode::Static(JsonNode::Paragraph(JsonParagraph {
            style_names: vec!["test_style".to_string()],
            children: vec![TemplateNode::Static(JsonNode::Text { content: "Hello".to_string() })],
            ..Default::default()
        }));
        let result = compiler.compile(&node).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            JsonInstruction::Paragraph { styles, children } => {
                assert_eq!(styles.static_styles.len(), 1);
                assert_eq!(styles.static_styles[0].font_size, Some(12.0));
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
        assert!(result.unwrap_err().to_string().contains("Style 'non_existent_style' not found"));
    }

    #[test]
    fn test_compile_if_then_else() {
        let (compiler, _) = create_test_compiler();
        let node: TemplateNode = serde_json::from_value(json!({
            "if": "show_it",
            "then": { "type": "Text", "content": "Then" },
            "else": { "type": "Text", "content": "Else" }
        }))
            .unwrap();
        let result = compiler.compile(&node).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            JsonInstruction::If { test, then_branch, else_branch } => {
                assert!(matches!(test, Expression::Selection(Selection::Path(_))));
                assert_eq!(then_branch.len(), 1);
                assert_eq!(else_branch.len(), 1);
            }
            _ => panic!("Expected If instruction"),
        }
    }
}