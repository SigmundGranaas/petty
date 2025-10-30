//! Implements the "Execution" phase for the JSON parser.
//! It walks the compiled instruction set and generates the `IRNode` tree.

use super::compiler::{
    CompiledStyles, CompiledString, CompiledTable, ExpressionPart, JsonInstruction,
};
use crate::core::idf::{
    IRNode, InlineMetadata, InlineNode, NodeMetadata, TableBody, TableCell, TableHeader, TableRow,
};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::parser::json::jpath::{self, engine, functions::FunctionRegistry};
use crate::parser::ParseError;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// A stateful executor that constructs an `IRNode` tree from a compiled instruction set.
pub struct TemplateExecutor<'s, 'd> {
    stylesheet: &'s Stylesheet,
    definitions: &'d HashMap<String, Vec<JsonInstruction>>,
    functions: FunctionRegistry,
    empty_vars: HashMap<String, Value>,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
}

impl<'s, 'd> TemplateExecutor<'s, 'd> {
    pub fn new(
        stylesheet: &'s Stylesheet,
        definitions: &'d HashMap<String, Vec<JsonInstruction>>,
    ) -> Self {
        Self {
            stylesheet,
            definitions,
            functions: FunctionRegistry::default(),
            empty_vars: HashMap::new(),
            node_stack: vec![],
            inline_stack: vec![],
        }
    }

    pub fn build_tree(
        &mut self,
        instructions: &[JsonInstruction],
        context: &Value,
    ) -> Result<Vec<IRNode>, ParseError> {
        self.node_stack.clear();
        self.inline_stack.clear();
        self.node_stack.push(IRNode::Root(Vec::with_capacity(16)));
        self.execute_instructions(instructions, context, None)?;
        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(ParseError::TemplateParse("Failed to construct root node.".to_string()))
        }
    }

    fn execute_instructions(
        &mut self,
        instructions: &[JsonInstruction],
        context: &Value,
        loop_pos: Option<usize>,
    ) -> Result<(), ParseError> {
        for instruction in instructions {
            self.execute_instruction(instruction, context, loop_pos)?;
        }
        Ok(())
    }

    fn get_eval_context<'a>(
        &'a self,
        context_node: &'a Value,
        loop_position: Option<usize>,
    ) -> engine::EvaluationContext<'a> {
        engine::EvaluationContext {
            context_node,
            variables: &self.empty_vars, // JSON parser does not currently use variables
            functions: &self.functions,
            loop_position,
        }
    }

    fn execute_instruction(
        &mut self,
        instruction: &JsonInstruction,
        context: &Value,
        loop_pos: Option<usize>,
    ) -> Result<(), ParseError> {
        match instruction {
            JsonInstruction::ForEach { select, body } => {
                let e_ctx = self.get_eval_context(context, loop_pos);
                let result_val = engine::evaluate(select, &e_ctx)?;
                let items = result_val.as_array().ok_or_else(|| {
                    ParseError::TemplateRender("#each path did not resolve to an array".to_string())
                })?;

                for (i, item_context) in items.iter().enumerate() {
                    self.execute_instructions(body, item_context, Some(i))?;
                }
            }
            JsonInstruction::If { test, then_branch, else_branch } => {
                let e_ctx = self.get_eval_context(context, loop_pos);
                let condition_met = engine::evaluate_as_bool(test, &e_ctx)?;

                if condition_met {
                    self.execute_instructions(then_branch, context, loop_pos)?;
                } else {
                    self.execute_instructions(else_branch, context, loop_pos)?;
                }
            }
            JsonInstruction::RenderTemplate { name } => {
                let template_instructions = self.definitions.get(name).ok_or_else(|| {
                    ParseError::TemplateParse(format!("Rendered template '{}' not found.", name))
                })?;
                self.execute_instructions(template_instructions, context, loop_pos)?;
            }
            JsonInstruction::PageBreak { master_name } => self
                .push_block_to_parent(IRNode::PageBreak { master_name: master_name.clone() }),
            JsonInstruction::Block { styles, children } => self.execute_container(IRNode::Block { meta: self.build_node_meta(styles, context, loop_pos)?, children: vec![] }, children, context, loop_pos)?,
            JsonInstruction::FlexContainer { styles, children } => self.execute_container(IRNode::FlexContainer { meta: self.build_node_meta(styles, context, loop_pos)?, children: vec![] }, children, context, loop_pos)?,
            JsonInstruction::List { styles, children } => self.execute_container(IRNode::List { meta: self.build_node_meta(styles, context, loop_pos)?, start: None, children: vec![] }, children, context, loop_pos)?,
            JsonInstruction::ListItem { styles, children } => self.execute_container(IRNode::ListItem { meta: self.build_node_meta(styles, context, loop_pos)?, children: vec![] }, children, context, loop_pos)?,
            JsonInstruction::Paragraph { styles, children } => {
                let para_node = IRNode::Paragraph {
                    meta: self.build_node_meta(styles, context, loop_pos)?,
                    children: vec![],
                };
                self.node_stack.push(para_node);
                self.execute_instructions(children, context, loop_pos)?;
                if let Some(completed_para) = self.node_stack.pop() {
                    self.push_block_to_parent(completed_para);
                }
            }
            JsonInstruction::Heading { level, styles, children } => {
                let heading_node = IRNode::Heading {
                    level: *level,
                    meta: self.build_node_meta(styles, context, loop_pos)?,
                    children: vec![],
                };
                self.node_stack.push(heading_node);
                self.execute_instructions(children, context, loop_pos)?;
                if let Some(completed_heading) = self.node_stack.pop() {
                    self.push_block_to_parent(completed_heading);
                }
            }
            JsonInstruction::TableOfContents { styles } => self
                .push_block_to_parent(IRNode::TableOfContents {
                    meta: self.build_node_meta(
                        styles,
                        context,
                        loop_pos,
                    )?,
                }),
            JsonInstruction::Image { styles, src } => self.push_block_to_parent(IRNode::Image { src: self.render_string(src, context, loop_pos)?, meta: self.build_node_meta(styles, context, loop_pos)? }),
            JsonInstruction::Table(table) => self.execute_table(table, context, loop_pos)?,
            JsonInstruction::Text { content } => self.push_inline_to_parent(InlineNode::Text(self.render_string(content, context, loop_pos)?)),
            JsonInstruction::LineBreak => self.push_inline_to_parent(InlineNode::LineBreak),
            JsonInstruction::InlineImage { styles, src } => self.push_inline_to_parent(InlineNode::Image { src: self.render_string(src, context, loop_pos)?, meta: self.build_inline_meta(styles, context, loop_pos)? }),
            JsonInstruction::StyledSpan { styles, children } => {
                self.inline_stack.push(InlineNode::StyledSpan { meta: self.build_inline_meta(styles, context, loop_pos)?, children: vec![] });
                self.execute_instructions(children, context, loop_pos)?;
                if let Some(s) = self.inline_stack.pop() {
                    self.push_inline_to_parent(s);
                }
            }
            JsonInstruction::Hyperlink { styles, href, children } => {
                self.inline_stack.push(InlineNode::Hyperlink { href: self.render_string(href, context, loop_pos)?, meta: self.build_inline_meta(styles, context, loop_pos)?, children: vec![] });
                self.execute_instructions(children, context, loop_pos)?;
                if let Some(h) = self.inline_stack.pop() {
                    self.push_inline_to_parent(h);
                }
            }
        }
        Ok(())
    }

    fn build_node_meta(&self, styles: &CompiledStyles, context: &Value, loop_pos: Option<usize>) -> Result<NodeMetadata, ParseError> {
        Ok(NodeMetadata {
            id: styles.id.clone(),
            style_sets: self.gather_styles(styles, context, loop_pos)?,
            style_override: styles.style_override.clone()
        })
    }

    fn build_inline_meta(&self, styles: &CompiledStyles, context: &Value, loop_pos: Option<usize>) -> Result<InlineMetadata, ParseError> {
        Ok(InlineMetadata {
            style_sets: self.gather_styles(styles, context, loop_pos)?,
            style_override: styles.style_override.clone()
        })
    }

    fn gather_styles(
        &self,
        styles: &CompiledStyles,
        context: &Value,
        loop_pos: Option<usize>,
    ) -> Result<Vec<Arc<ElementStyle>>, ParseError> {
        let mut resolved_styles = styles.static_styles.clone();
        for name_template in &styles.dynamic_style_templates {
            for name in self
                .render_string(name_template, context, loop_pos)?
                .split_whitespace()
                .filter(|s| !s.is_empty())
            {
                resolved_styles.push(self.stylesheet.styles.get(name).cloned().ok_or_else(
                    || {
                        ParseError::TemplateParse(format!(
                            "Style '{}' (rendered) not found",
                            name
                        ))
                    },
                )?);
            }
        }
        Ok(resolved_styles)
    }

    fn execute_container(
        &mut self,
        mut node: IRNode,
        children: &[JsonInstruction],
        context: &Value,
        loop_pos: Option<usize>,
    ) -> Result<(), ParseError> {
        // Create a new executor to build a sub-tree in isolation, preserving the current stack.
        let mut sub_executor = TemplateExecutor::new(self.stylesheet, self.definitions);

        // Manually set up the sub-executor and call execute_instructions to pass loop_pos.
        sub_executor.node_stack.push(IRNode::Root(Vec::new()));
        sub_executor.execute_instructions(children, context, loop_pos)?;
        let child_nodes = if let Some(IRNode::Root(nodes)) = sub_executor.node_stack.pop() {
            nodes
        } else {
            return Err(ParseError::TemplateParse("Failed to build sub-tree for container.".to_string()));
        };

        match &mut node {
            IRNode::Block { children: c, .. }
            | IRNode::FlexContainer { children: c, .. }
            | IRNode::List { children: c, .. }
            | IRNode::ListItem { children: c, .. } => *c = child_nodes,
            _ => {} // Should not happen for containers
        }
        self.push_block_to_parent(node);
        Ok(())
    }

    fn execute_table(
        &mut self,
        table: &CompiledTable,
        context: &Value,
        loop_pos: Option<usize>,
    ) -> Result<(), ParseError> {
        let header = if let Some(instructions) = &table.header {
            let mut sub_executor = TemplateExecutor::new(self.stylesheet, self.definitions);
            Some(Box::new(TableHeader {
                rows: sub_executor
                    .build_tree(instructions, context)?
                    .into_iter()
                    .map(TableRow::try_from)
                    .collect::<Result<_, _>>()?,
            }))
        } else {
            None
        };
        let mut sub_executor = TemplateExecutor::new(self.stylesheet, self.definitions);
        let body = Box::new(TableBody {
            rows: sub_executor
                .build_tree(&table.body, context)?
                .into_iter()
                .map(TableRow::try_from)
                .collect::<Result<_, _>>()?,
        });
        self.push_block_to_parent(IRNode::Table {
            meta: self.build_node_meta(&table.styles, context, loop_pos)?,
            columns: table.columns.clone(),
            header,
            body,
        });
        Ok(())
    }

    fn render_string(
        &self,
        compiled_str: &CompiledString,
        context: &Value,
        loop_pos: Option<usize>,
    ) -> Result<String, ParseError> {
        match compiled_str {
            CompiledString::Static(s) => Ok(s.clone()),
            CompiledString::Dynamic(parts) => {
                let mut result = String::new();
                let e_ctx = self.get_eval_context(context, loop_pos);
                for part in parts {
                    match part {
                        ExpressionPart::Static(s) => result.push_str(s),
                        ExpressionPart::Dynamic(expression) => {
                            let s = jpath::evaluate_as_string(expression, &e_ctx)?;
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
                IRNode::Root(c)
                | IRNode::Block { children: c, .. }
                | IRNode::FlexContainer { children: c, .. }
                | IRNode::List { children: c, .. }
                | IRNode::ListItem { children: c, .. } => c.push(node),
                _ => log::warn!("Cannot add block node to current parent: {:?}", parent),
            }
        }
    }

    fn push_inline_to_parent(&mut self, node: InlineNode) {
        if let Some(parent_inline) = self.inline_stack.last_mut() {
            match parent_inline {
                InlineNode::StyledSpan { children: c, .. }
                | InlineNode::Hyperlink { children: c, .. }
                | InlineNode::PageReference { children: c, .. } => {
                    c.push(node);
                    return;
                }
                _ => {}
            }
        }
        if let Some(parent_block) = self.node_stack.last_mut() {
            match parent_block {
                IRNode::Paragraph { children: c, .. }
                | IRNode::Heading { children: c, .. } => {
                    c.push(node);
                    return;
                }
                _ => {}
            }
        }

        self.push_block_to_parent(IRNode::Paragraph {
            meta: Default::default(),
            children: vec![node],
        });
    }
}

// --- TryFrom Implementations required for table/inline processing ---
impl TryFrom<IRNode> for TableRow {
    type Error = ParseError;
    fn try_from(node: IRNode) -> Result<Self, Self::Error> {
        if let IRNode::Block { children, .. } = node {
            Ok(TableRow {
                cells: children.into_iter().map(TableCell::try_from).collect::<Result<_, _>>()?,
            })
        } else {
            Err(ParseError::TemplateParse(format!(
                "Expected Block to convert to TableRow, got {:?}",
                node
            )))
        }
    }
}
impl TryFrom<IRNode> for TableCell {
    type Error = ParseError;
    fn try_from(node: IRNode) -> Result<Self, Self::Error> {
        if let IRNode::Block { meta, children } = node {
            Ok(TableCell {
                style_sets: meta.style_sets,
                style_override: meta.style_override,
                children,
                colspan: 1,
                rowspan: 1,
            })
        } else {
            Err(ParseError::TemplateParse(format!(
                "Expected Block to convert to TableCell, got {:?}",
                node
            )))
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
                    Err(ParseError::TemplateParse(
                        "Cannot convert multi-child paragraph to single inline node.".into(),
                    ))
                }
            }
            _ => Err(ParseError::TemplateParse("Node cannot be converted to an InlineNode".into())),
        }
    }
}