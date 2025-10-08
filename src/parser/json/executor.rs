//! Implements the "Execution" phase for the JSON parser.
//! It walks the compiled instruction set and generates the `IRNode` tree.

use super::compiler::{CompiledStyles, CompiledString, CompiledTable, ExpressionPart, JsonInstruction};
use crate::core::idf::{IRNode, InlineNode, TableBody, TableCell, TableHeader, TableRow};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::parser::ParseError;
use crate::xpath;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// A stateful executor that constructs an `IRNode` tree from a compiled instruction set.
pub struct TemplateExecutor<'s, 'd> {
    stylesheet: &'s Stylesheet,
    definitions: &'d HashMap<String, Vec<JsonInstruction>>,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
}

impl<'s, 'd> TemplateExecutor<'s, 'd> {
    pub fn new(stylesheet: &'s Stylesheet, definitions: &'d HashMap<String, Vec<JsonInstruction>>) -> Self {
        Self {
            stylesheet,
            definitions,
            node_stack: vec![],
            inline_stack: vec![],
        }
    }

    pub fn build_tree(&mut self, instructions: &[JsonInstruction], context: &Value) -> Result<Vec<IRNode>, ParseError> {
        self.node_stack.clear();
        self.inline_stack.clear();
        self.node_stack.push(IRNode::Root(Vec::with_capacity(16)));
        self.execute_instructions(instructions, context)?;
        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(ParseError::TemplateParse("Failed to construct root node.".to_string()))
        }
    }

    fn execute_instructions(&mut self, instructions: &[JsonInstruction], context: &Value) -> Result<(), ParseError> {
        for instruction in instructions {
            self.execute_instruction(instruction, context)?;
        }
        Ok(())
    }

    fn execute_instruction(&mut self, instruction: &JsonInstruction, context: &Value) -> Result<(), ParseError> {
        match instruction {
            JsonInstruction::ForEach { select, body } => {
                let variables = HashMap::new(); // `each` doesn't currently use variables.
                let items = select.select(context, &variables).first().and_then(|v| v.as_array()).ok_or_else(|| ParseError::TemplateRender("#each path did not resolve to an array".to_string()))?;

                for item_context in items {
                    self.execute_instructions(body, item_context)?;
                }
            }
            JsonInstruction::If { test, then_branch, else_branch } => {
                let variables = HashMap::new();
                let results = test.select(context, &variables);
                // Truthiness check: non-empty, and first element is not null or `false`.
                let condition_met = !results.is_empty()
                    && results.iter().all(|v| !v.is_null() && v.as_bool() != Some(false));

                if condition_met {
                    self.execute_instructions(then_branch, context)?;
                } else {
                    self.execute_instructions(else_branch, context)?;
                }
            }
            JsonInstruction::RenderTemplate { name } => {
                let template_instructions = self.definitions.get(name).ok_or_else(|| ParseError::TemplateParse(format!("Rendered template '{}' not found.", name)))?;
                self.execute_instructions(template_instructions, context)?;
            }
            JsonInstruction::PageBreak { master_name } => self.push_block_to_parent(IRNode::PageBreak { master_name: master_name.clone() }),
            JsonInstruction::Block { styles, children } => self.execute_container(IRNode::Block { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::FlexContainer { styles, children } => self.execute_container(IRNode::FlexContainer { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::List { styles, children } => self.execute_container(IRNode::List { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), start: None, children: vec![] }, children, context)?,
            JsonInstruction::ListItem { styles, children } => self.execute_container(IRNode::ListItem { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::Paragraph { styles, children } => {
                let para_node = IRNode::Paragraph {
                    style_sets: self.gather_styles(styles, context)?,
                    style_override: styles.style_override.clone(),
                    children: vec![],
                };
                self.node_stack.push(para_node);
                self.execute_instructions(children, context)?;
                if let Some(completed_para) = self.node_stack.pop() {
                    self.push_block_to_parent(completed_para);
                }
            }
            JsonInstruction::Image { styles, src } => self.push_block_to_parent(IRNode::Image { src: self.render_string(src, context)?, style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone() }),
            JsonInstruction::Table(table) => self.execute_table(table, context)?,
            JsonInstruction::Text { content } => self.push_inline_to_parent(InlineNode::Text(self.render_string(content, context)?)),
            JsonInstruction::LineBreak => self.push_inline_to_parent(InlineNode::LineBreak),
            JsonInstruction::InlineImage { styles, src } => self.push_inline_to_parent(InlineNode::Image { src: self.render_string(src, context)?, style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone() }),
            JsonInstruction::StyledSpan { styles, children } => {
                self.inline_stack.push(InlineNode::StyledSpan { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] });
                self.execute_instructions(children, context)?;
                if let Some(s) = self.inline_stack.pop() {
                    self.push_inline_to_parent(s);
                }
            }
            JsonInstruction::Hyperlink { styles, href, children } => {
                self.inline_stack.push(InlineNode::Hyperlink { href: self.render_string(href, context)?, style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] });
                self.execute_instructions(children, context)?;
                if let Some(h) = self.inline_stack.pop() {
                    self.push_inline_to_parent(h);
                }
            }
        }
        Ok(())
    }

    fn gather_styles(&self, styles: &CompiledStyles, context: &Value) -> Result<Vec<Arc<ElementStyle>>, ParseError> {
        let mut resolved_styles = styles.static_styles.clone();
        for name_template in &styles.dynamic_style_templates {
            for name in self.render_string(name_template, context)?.split_whitespace().filter(|s| !s.is_empty()) {
                resolved_styles.push(self.stylesheet.styles.get(name).cloned().ok_or_else(|| ParseError::TemplateParse(format!("Style '{}' (rendered) not found", name)))?);
            }
        }
        Ok(resolved_styles)
    }

    fn execute_container(&mut self, mut node: IRNode, children: &[JsonInstruction], context: &Value) -> Result<(), ParseError> {
        let mut sub_executor = TemplateExecutor::new(self.stylesheet, self.definitions);
        let child_nodes = sub_executor.build_tree(children, context)?;
        match &mut node {
            IRNode::Block { children: c, .. }
            | IRNode::FlexContainer { children: c, .. }
            | IRNode::List { children: c, .. }
            | IRNode::ListItem { children: c, .. } => *c = child_nodes,
            _ => {}
        }
        self.push_block_to_parent(node);
        Ok(())
    }

    fn execute_table(&mut self, table: &CompiledTable, context: &Value) -> Result<(), ParseError> {
        let header = if let Some(instructions) = &table.header {
            let mut sub_executor = TemplateExecutor::new(self.stylesheet, self.definitions);
            Some(Box::new(TableHeader { rows: sub_executor.build_tree(instructions, context)?.into_iter().map(TableRow::try_from).collect::<Result<_, _>>()? }))
        } else {
            None
        };
        let mut sub_executor = TemplateExecutor::new(self.stylesheet, self.definitions);
        let body = Box::new(TableBody { rows: sub_executor.build_tree(&table.body, context)?.into_iter().map(TableRow::try_from).collect::<Result<_, _>>()? });
        self.push_block_to_parent(IRNode::Table { style_sets: self.gather_styles(&table.styles, context)?, style_override: table.styles.style_override.clone(), columns: table.columns.clone(), header, body });
        Ok(())
    }

    fn render_string(&self, compiled_str: &CompiledString, context: &Value) -> Result<String, ParseError> {
        match compiled_str {
            CompiledString::Static(s) => Ok(s.clone()),
            CompiledString::Dynamic(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        ExpressionPart::Static(s) => result.push_str(s),
                        ExpressionPart::Dynamic(pointer) => {
                            let s = xpath::select_as_string(&xpath::Selection::JsonPointer(pointer.clone()), context, &HashMap::new());
                            result.push_str(&s);
                        }
                    }
                }
                Ok(result)
            }
        }
    }

    fn push_block_to_parent(&mut self, node: IRNode) {
        if let Some(parent) = self.node_stack.last_mut() {
            match parent {
                IRNode::Root(c) | IRNode::Block { children: c, .. } | IRNode::FlexContainer { children: c, .. } | IRNode::List { children: c, .. } | IRNode::ListItem { children: c, .. } => c.push(node),
                _ => log::warn!("Cannot add block node to current parent: {:?}", parent),
            }
        }
    }

    fn push_inline_to_parent(&mut self, node: InlineNode) {
        if let Some(parent_inline) = self.inline_stack.last_mut() {
            if let InlineNode::StyledSpan { children: c, .. } | InlineNode::Hyperlink { children: c, .. } = parent_inline {
                c.push(node);
                return;
            }
        }
        if let Some(IRNode::Paragraph { children: c, .. }) = self.node_stack.last_mut() {
            c.push(node);
        } else {
            self.push_block_to_parent(IRNode::Paragraph { style_sets: vec![], style_override: None, children: vec![node] });
        }
    }
}

// --- TryFrom Implementations required for table/inline processing ---
impl TryFrom<IRNode> for TableRow {
    type Error = ParseError;
    fn try_from(node: IRNode) -> Result<Self, Self::Error> {
        if let IRNode::Block { children, .. } = node {
            Ok(TableRow { cells: children.into_iter().map(TableCell::try_from).collect::<Result<_, _>>()? })
        } else {
            Err(ParseError::TemplateParse(format!("Expected Block to convert to TableRow, got {:?}", node)))
        }
    }
}
impl TryFrom<IRNode> for TableCell {
    type Error = ParseError;
    fn try_from(node: IRNode) -> Result<Self, Self::Error> {
        if let IRNode::Block { style_sets, style_override, children } = node {
            Ok(TableCell { style_sets, style_override, children, colspan: 1, rowspan: 1})
        } else {
            Err(ParseError::TemplateParse(format!("Expected Block to convert to TableCell, got {:?}", node)))
        }
    }
}
impl TryFrom<IRNode> for InlineNode {
    type Error = ParseError;
    fn try_from(node: IRNode) -> Result<Self, Self::Error> {
        match node {
            IRNode::Paragraph { mut children, .. } => {
                if children.len() == 1 {
                    Ok(children.remove(0))
                } else if children.is_empty() {
                    Ok(InlineNode::Text("".to_string()))
                } else {
                    Err(ParseError::TemplateParse("Cannot convert multi-child paragraph to single inline node.".into()))
                }
            }
            _ => Err(ParseError::TemplateParse("Node cannot be converted to an InlineNode".into())),
        }
    }
}