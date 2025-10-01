//! Implements the "Execution" phase for the JSON parser.
//! It walks the compiled instruction set and generates the `IRNode` tree.

use super::compiler::{CompiledStyles, CompiledTable, JsonInstruction};
use crate::core::idf::{IRNode, InlineNode, TableBody, TableCell, TableHeader, TableRow};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::parser::ParseError;
use handlebars::Handlebars;
use serde_json::Value;
use std::sync::Arc;

/// A stateful executor that constructs an `IRNode` tree from a compiled instruction set.
pub struct TemplateExecutor<'h, 's> {
    template_engine: &'h Handlebars<'h>,
    stylesheet: &'s Stylesheet,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
}

impl<'h, 's> TemplateExecutor<'h, 's> {
    pub fn new(template_engine: &'h Handlebars<'h>, stylesheet: &'s Stylesheet) -> Self {
        Self { template_engine, stylesheet, node_stack: vec![], inline_stack: vec![] }
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
        for instruction in instructions { self.execute_instruction(instruction, context)?; }
        Ok(())
    }

    fn execute_instruction(&mut self, instruction: &JsonInstruction, context: &Value) -> Result<(), ParseError> {
        match instruction {
            JsonInstruction::ForEach { in_path, body } => {
                let pointer_path = format!("/{}", in_path.replace('.', "/"));
                let data_to_iterate = context.pointer(&pointer_path).ok_or_else(|| ParseError::TemplateRender(format!("#each path '{}' not found", in_path)))?;
                let items = data_to_iterate.as_array().ok_or_else(|| ParseError::TemplateRender(format!("#each path '{}' not an array", in_path)))?;
                for item_context in items { self.execute_instructions(body, item_context)?; }
            }
            JsonInstruction::If { test, then_branch, else_branch } => {
                let condition_template = format!("{{{{#if {}}}}}true{{{{/if}}}}", test.trim_matches(|c| c == '{' || c == '}'));
                if self.render_text(&condition_template, context)? == "true" {
                    self.execute_instructions(then_branch, context)?;
                } else {
                    self.execute_instructions(else_branch, context)?;
                }
            }
            JsonInstruction::Block { styles, children } => self.execute_container(IRNode::Block { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::FlexContainer { styles, children } => self.execute_container(IRNode::FlexContainer { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::List { styles, children } => self.execute_container(IRNode::List { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::ListItem { styles, children } => self.execute_container(IRNode::ListItem { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::Paragraph { styles, children } => self.execute_container(IRNode::Paragraph { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] }, children, context)?,
            JsonInstruction::Image { styles, src_template } => self.push_block_to_parent(IRNode::Image { src: self.render_text(src_template, context)?, style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone() }),
            JsonInstruction::Table(table) => self.execute_table(table, context)?,
            JsonInstruction::Text { content_template } => self.push_inline_to_parent(InlineNode::Text(self.render_text(content_template, context)?)),
            JsonInstruction::LineBreak => self.push_inline_to_parent(InlineNode::LineBreak),
            JsonInstruction::InlineImage { styles, src_template } => self.push_inline_to_parent(InlineNode::Image { src: self.render_text(src_template, context)?, style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone() }),
            JsonInstruction::StyledSpan { styles, children } => {
                self.inline_stack.push(InlineNode::StyledSpan { style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] });
                self.execute_instructions(children, context)?;
                if let Some(s) = self.inline_stack.pop() { self.push_inline_to_parent(s); }
            }
            JsonInstruction::Hyperlink { styles, href_template, children } => {
                self.inline_stack.push(InlineNode::Hyperlink { href: self.render_text(href_template, context)?, style_sets: self.gather_styles(styles, context)?, style_override: styles.style_override.clone(), children: vec![] });
                self.execute_instructions(children, context)?;
                if let Some(h) = self.inline_stack.pop() { self.push_inline_to_parent(h); }
            }
        }
        Ok(())
    }

    fn gather_styles(&self, styles: &CompiledStyles, context: &Value) -> Result<Vec<Arc<ElementStyle>>, ParseError> {
        let mut resolved_styles = styles.static_styles.clone();
        for name_template in &styles.dynamic_style_templates {
            for name in self.render_text(name_template, context)?.split_whitespace().filter(|s| !s.is_empty()) {
                resolved_styles.push(self.stylesheet.styles.get(name).cloned().ok_or_else(|| ParseError::TemplateParse(format!("Style '{}' (rendered) not found", name)))?);
            }
        }
        Ok(resolved_styles)
    }

    fn execute_container(&mut self, mut node: IRNode, children: &[JsonInstruction], context: &Value) -> Result<(), ParseError> {
        let mut sub_executor = TemplateExecutor::new(self.template_engine, self.stylesheet);
        let child_nodes = sub_executor.build_tree(children, context)?;
        match &mut node {
            IRNode::Block { children: c, .. } | IRNode::FlexContainer { children: c, .. } | IRNode::List { children: c, .. } | IRNode::ListItem { children: c, .. } => *c = child_nodes,
            IRNode::Paragraph { children: c, .. } => *c = child_nodes.into_iter().map(InlineNode::try_from).collect::<Result<_, _>>()?,
            _ => {}
        }
        self.push_block_to_parent(node);
        Ok(())
    }

    fn execute_table(&mut self, table: &CompiledTable, context: &Value) -> Result<(), ParseError> {
        let header = if let Some(instructions) = &table.header {
            let mut sub_executor = TemplateExecutor::new(self.template_engine, self.stylesheet);
            Some(Box::new(TableHeader { rows: sub_executor.build_tree(instructions, context)?.into_iter().map(TableRow::try_from).collect::<Result<_, _>>()? }))
        } else { None };
        let mut sub_executor = TemplateExecutor::new(self.template_engine, self.stylesheet);
        let body = Box::new(TableBody { rows: sub_executor.build_tree(&table.body, context)?.into_iter().map(TableRow::try_from).collect::<Result<_, _>>()? });
        self.push_block_to_parent(IRNode::Table { style_sets: self.gather_styles(&table.styles, context)?, style_override: table.styles.style_override.clone(), columns: table.columns.clone(), header, body });
        Ok(())
    }

    fn render_text(&self, text: &str, context: &Value) -> Result<String, ParseError> {
        if !text.contains("{{") { return Ok(text.to_string()); }
        self.template_engine.render_template(text, context).map_err(|e| ParseError::TemplateRender(e.to_string()))
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
            if let InlineNode::StyledSpan { children: c, .. } | InlineNode::Hyperlink { children: c, .. } = parent_inline { c.push(node); return; }
        }
        if let Some(IRNode::Paragraph { children: c, .. }) = self.node_stack.last_mut() {
            c.push(node);
        } else {
            self.push_block_to_parent(IRNode::Paragraph { style_sets: vec![], style_override: None, children: vec![node] });
        }
    }
}

// --- TryFrom Implementations required for table/inline processing ---
impl TryFrom<IRNode> for TableRow { type Error = ParseError; fn try_from(node: IRNode) -> Result<Self, Self::Error> { if let IRNode::Block { children, .. } = node { Ok(TableRow { cells: children.into_iter().map(TableCell::try_from).collect::<Result<_, _>>()? }) } else { Err(ParseError::TemplateParse(format!("Expected Block to convert to TableRow, got {:?}", node))) } } }
impl TryFrom<IRNode> for TableCell { type Error = ParseError; fn try_from(node: IRNode) -> Result<Self, Self::Error> { if let IRNode::Block { style_sets, style_override, children } = node { Ok(TableCell { style_sets, style_override, children }) } else { Err(ParseError::TemplateParse(format!("Expected Block to convert to TableCell, got {:?}", node))) } } }
impl TryFrom<IRNode> for InlineNode { type Error = ParseError; fn try_from(node: IRNode) -> Result<Self, Self::Error> { match node { IRNode::Paragraph { mut children, .. } => { if children.len() == 1 { Ok(children.remove(0)) } else if children.is_empty() { Ok(InlineNode::Text("".to_string())) } else { Err(ParseError::TemplateParse("Cannot convert multi-child paragraph to single inline node.".into())) } } _ => Err(ParseError::TemplateParse("Node cannot be converted to an InlineNode".into())), } } }