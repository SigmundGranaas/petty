//! Implements the "Execution" phase for the JSON parser.
//! It walks the compiled instruction set and generates the `IRNode` tree.

use super::compiler::{CompiledStyles, CompiledTable, JsonInstruction};
use crate::parser::ParseError;
use handlebars::Handlebars;
use std::sync::Arc;
use serde_json::Value;
use crate::core::idf::{IRNode, InlineNode, TableBody, TableCell, TableHeader, TableRow};
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};

/// A stateful executor that constructs an `IRNode` tree from a compiled instruction set.
pub struct TemplateExecutor<'h, 's> {
    template_engine: &'h Handlebars<'static>,
    stylesheet: &'s Stylesheet,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
}

impl<'h, 's> TemplateExecutor<'h, 's> {
    /// Creates a new executor.
    pub fn new(template_engine: &'h Handlebars<'static>, stylesheet: &'s Stylesheet) -> Self {
        Self {
            template_engine,
            stylesheet,
            node_stack: vec![],
            inline_stack: vec![],
        }
    }

    /// The main entry point. Executes a set of instructions against a data context.
    pub fn build_tree(
        &mut self,
        instructions: &[JsonInstruction],
        context: &Value,
    ) -> Result<Vec<IRNode>, ParseError> {
        self.node_stack.clear();
        self.inline_stack.clear();

        // Create a temporary root to hold the results of this execution
        let root_node = IRNode::Root(Vec::with_capacity(16));
        self.node_stack.push(root_node);

        self.execute_instructions(instructions, context)?;

        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            // This case should be unreachable if the logic is sound
            Err(ParseError::TemplateParse(
                "Failed to construct root node.".to_string(),
            ))
        }
    }

    /// Recursively walks the instruction set and builds the `IRNode` tree.
    fn execute_instructions(
        &mut self,
        instructions: &[JsonInstruction],
        context: &Value,
    ) -> Result<(), ParseError> {
        for instruction in instructions {
            self.execute_instruction(instruction, context)?;
        }
        Ok(())
    }

    /// Executes a single instruction.
    fn execute_instruction(
        &mut self,
        instruction: &JsonInstruction,
        context: &Value,
    ) -> Result<(), ParseError> {
        match instruction {
            // Control Flow
            JsonInstruction::ForEach { in_path, body } => {
                let pointer_path = format!("/{}", in_path.replace('.', "/"));
                let data_to_iterate = context.pointer(&pointer_path).ok_or_else(|| {
                    ParseError::TemplateRender(format!("#each path '{}' not found in data", in_path))
                })?;
                let items = data_to_iterate.as_array().ok_or_else(|| {
                    ParseError::TemplateRender(format!(
                        "#each path '{}' did not resolve to an array",
                        in_path
                    ))
                })?;

                for item_context in items {
                    self.execute_instructions(body, item_context)?;
                }
            }
            JsonInstruction::If {
                test,
                then_branch,
                else_branch,
            } => {
                // Handlebars has its own complex truthiness logic that we can't easily replicate.
                // Instead, we can wrap the condition in a template and see if it renders anything.
                let condition_template = format!("{{{{#if {}}}}}true{{{{/if}}}}", test.trim_matches(|c| c == '{' || c == '}'));
                let rendered_test = self.render_text(&condition_template, context)?;

                if rendered_test == "true" {
                    self.execute_instructions(then_branch, context)?;
                } else {
                    self.execute_instructions(else_branch, context)?;
                }
            }

            // Block-level
            JsonInstruction::Block { styles, children } => {
                let style_sets = self.gather_styles(styles, context)?;
                self.execute_container(
                    IRNode::Block {
                        style_sets,
                        style_override: styles.style_override.clone(),
                        children: vec![],
                    },
                    children,
                    context,
                )?
            }
            JsonInstruction::FlexContainer { styles, children } => {
                let style_sets = self.gather_styles(styles, context)?;
                self.execute_container(
                    IRNode::FlexContainer {
                        style_sets,
                        style_override: styles.style_override.clone(),
                        children: vec![],
                    },
                    children,
                    context,
                )?
            }
            JsonInstruction::List { styles, children } => {
                let style_sets = self.gather_styles(styles, context)?;
                self.execute_container(
                    IRNode::List {
                        style_sets,
                        style_override: styles.style_override.clone(),
                        children: vec![],
                    },
                    children,
                    context,
                )?
            }
            JsonInstruction::ListItem { styles, children } => {
                let style_sets = self.gather_styles(styles, context)?;
                self.execute_container(
                    IRNode::ListItem {
                        style_sets,
                        style_override: styles.style_override.clone(),
                        children: vec![],
                    },
                    children,
                    context,
                )?
            }
            JsonInstruction::Paragraph { styles, children } => {
                let style_sets = self.gather_styles(styles, context)?;
                self.execute_container(
                    IRNode::Paragraph {
                        style_sets,
                        style_override: styles.style_override.clone(),
                        children: vec![],
                    },
                    children,
                    context,
                )?
            }
            JsonInstruction::Image {
                styles,
                src_template,
            } => {
                let src = self.render_text(src_template, context)?;
                let style_sets = self.gather_styles(styles, context)?;
                let node = IRNode::Image {
                    src,
                    style_sets,
                    style_override: styles.style_override.clone(),
                };
                self.push_block_to_parent(node);
            }
            JsonInstruction::Table(table) => {
                self.execute_table(table, context)?;
            }

            // Inline-level
            JsonInstruction::Text { content_template } => {
                let content = self.render_text(content_template, context)?;
                self.push_inline_to_parent(InlineNode::Text(content));
            }
            JsonInstruction::LineBreak => self.push_inline_to_parent(InlineNode::LineBreak),
            JsonInstruction::InlineImage {
                styles,
                src_template,
            } => {
                let src = self.render_text(src_template, context)?;
                let style_sets = self.gather_styles(styles, context)?;
                let node = InlineNode::Image {
                    src,
                    style_sets,
                    style_override: styles.style_override.clone(),
                };
                self.push_inline_to_parent(node);
            }
            JsonInstruction::StyledSpan { styles, children } => {
                let style_sets = self.gather_styles(styles, context)?;
                let node = InlineNode::StyledSpan {
                    style_sets,
                    style_override: styles.style_override.clone(),
                    children: vec![],
                };
                self.inline_stack.push(node);
                self.execute_instructions(children, context)?;
                if let Some(s) = self.inline_stack.pop() {
                    self.push_inline_to_parent(s);
                }
            }
            JsonInstruction::Hyperlink {
                styles,
                href_template,
                children,
            } => {
                let href = self.render_text(href_template, context)?;
                let style_sets = self.gather_styles(styles, context)?;
                let node = InlineNode::Hyperlink {
                    href,
                    style_sets,
                    style_override: styles.style_override.clone(),
                    children: vec![],
                };
                self.inline_stack.push(node);
                self.execute_instructions(children, context)?;
                if let Some(h) = self.inline_stack.pop() {
                    self.push_inline_to_parent(h);
                }
            }
        }
        Ok(())
    }

    /// Gathers final styles from pre-compiled static and dynamic templates.
    fn gather_styles(
        &self,
        styles: &CompiledStyles,
        context: &Value,
    ) -> Result<Vec<Arc<ElementStyle>>, ParseError> {
        // Start with the pre-resolved static styles.
        let mut resolved_styles = styles.static_styles.clone();

        // Render and resolve the dynamic style templates.
        for name_template in &styles.dynamic_style_templates {
            let rendered_names = self.render_text(name_template, context)?;
            for name in rendered_names.split_whitespace() {
                if !name.is_empty() {
                    let style = self.stylesheet.styles.get(name).cloned().ok_or_else(|| {
                        ParseError::TemplateParse(format!(
                            "Style '{}' (rendered from template) not found in stylesheet",
                            name
                        ))
                    })?;
                    resolved_styles.push(style);
                }
            }
        }
        Ok(resolved_styles)
    }

    /// Helper to execute a generic container node.
    fn execute_container(
        &mut self,
        mut node: IRNode,
        children: &[JsonInstruction],
        context: &Value,
    ) -> Result<(), ParseError> {
        // Create a new executor for the sub-tree to avoid clobbering the current stack.
        let mut sub_executor = TemplateExecutor::new(self.template_engine, self.stylesheet);
        let child_nodes = sub_executor.build_tree(children, context)?;

        match &mut node {
            IRNode::Block { children, .. }
            | IRNode::FlexContainer { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => {
                *children = child_nodes;
            }
            IRNode::Paragraph { children, .. } => {
                *children = child_nodes
                    .into_iter()
                    .map(InlineNode::try_from)
                    .collect::<Result<_, _>>()?;
            }
            _ => { /* Other node types don't have generic children */ }
        }
        self.push_block_to_parent(node);
        Ok(())
    }

    /// Special-cased executor for the complex `Table` node.
    fn execute_table(
        &mut self,
        table: &CompiledTable,
        context: &Value,
    ) -> Result<(), ParseError> {
        let header = if let Some(instructions) = &table.header {
            let mut sub_executor = TemplateExecutor::new(self.template_engine, self.stylesheet);
            let header_rows_ir = sub_executor.build_tree(instructions, context)?;
            Some(Box::new(TableHeader {
                rows: header_rows_ir
                    .into_iter()
                    .map(TableRow::try_from)
                    .collect::<Result<_, _>>()?,
            }))
        } else {
            None
        };

        let mut sub_executor = TemplateExecutor::new(self.template_engine, self.stylesheet);
        let body_rows_ir = sub_executor.build_tree(&table.body, context)?;
        let body = Box::new(TableBody {
            rows: body_rows_ir
                .into_iter()
                .map(TableRow::try_from)
                .collect::<Result<_, _>>()?,
        });

        let style_sets = self.gather_styles(&table.styles, context)?;
        let table_node = IRNode::Table {
            style_sets,
            style_override: table.styles.style_override.clone(),
            columns: table.columns.clone(),
            calculated_widths: vec![],
            header,
            body,
        };
        self.push_block_to_parent(table_node);

        Ok(())
    }

    /// Renders a Handlebars template string.
    fn render_text(&self, text: &str, context: &Value) -> Result<String, ParseError> {
        if !text.contains("{{") {
            return Ok(text.to_string());
        }
        self.template_engine
            .render_template(text, context)
            .map_err(|e| ParseError::TemplateRender(e.to_string()))
    }

    /// Pushes a block node to the children of the current node on the stack.
    fn push_block_to_parent(&mut self, node: IRNode) {
        if let Some(parent) = self.node_stack.last_mut() {
            match parent {
                IRNode::Root(children)
                | IRNode::Block { children, .. }
                | IRNode::FlexContainer { children, .. }
                | IRNode::List { children, .. }
                | IRNode::ListItem { children, .. } => children.push(node),
                // Tables are handled by execute_table, so no push logic is needed here
                _ => log::warn!("Cannot add block node to current parent: {:?}", parent),
            }
        }
    }

    /// Pushes an inline node to the current inline or paragraph context.
    fn push_inline_to_parent(&mut self, node: InlineNode) {
        if let Some(parent_inline) = self.inline_stack.last_mut() {
            match parent_inline {
                InlineNode::StyledSpan { children, .. }
                | InlineNode::Hyperlink { children, .. } => {
                    children.push(node);
                    return;
                }
                _ => {}
            }
        }

        // If not in an inline, find the nearest paragraph or create one implicitly.
        if let Some(IRNode::Paragraph { children, .. }) = self.node_stack.last_mut() {
            children.push(node);
        } else {
            // Implicitly create a paragraph to hold the inline content
            let paragraph = IRNode::Paragraph {
                style_sets: vec![],
                style_override: None,
                children: vec![node],
            };
            self.push_block_to_parent(paragraph);
        }
    }
}

// --- TryFrom Implementations required for table/inline processing ---

impl TryFrom<IRNode> for TableRow {
    type Error = ParseError;
    fn try_from(node: IRNode) -> Result<Self, Self::Error> {
        if let IRNode::Block { children, .. } = node {
            Ok(TableRow {
                cells: children
                    .into_iter()
                    .map(TableCell::try_from)
                    .collect::<Result<_, _>>()?,
            })
        } else {
            Err(ParseError::TemplateParse(format!(
                "Expected a Block node to convert to TableRow, but got {:?}",
                node
            )))
        }
    }
}
impl TryFrom<IRNode> for TableCell {
    type Error = ParseError;
    fn try_from(node: IRNode) -> Result<Self, Self::Error> {
        if let IRNode::Block {
            style_sets,
            style_override,
            children,
        } = node
        {
            Ok(TableCell {
                style_sets,
                style_override,
                children,
            })
        } else {
            Err(ParseError::TemplateParse(format!(
                "Expected a Block node to convert to TableCell, but got {:?}",
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
            _ => Err(ParseError::TemplateParse(
                "Node cannot be converted to an InlineNode".into(),
            )),
        }
    }
}