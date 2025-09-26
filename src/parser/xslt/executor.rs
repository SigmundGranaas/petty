use super::ast::{
    CompiledStylesheet, PreparsedStyles, PreparsedTemplate, TableColumnDefinition, TemplateRule,
    XsltInstruction,
};
use crate::parser::ParseError;
use crate::xpath;
use handlebars::Handlebars;
use serde_json::Value;
use std::collections::HashMap;
use crate::core::idf;
use crate::core::idf::IRNode::Table;
use crate::core::idf::{IRNode, InlineNode, TableBody, TableCell, TableHeader, TableRow};

/// A stateful executor that constructs an `IRNode` tree by processing a `CompiledStylesheet`
/// against a JSON data context. It implements the XSLT "push" model.
pub struct TemplateExecutor<'h> {
    template_engine: &'h Handlebars<'static>,
    stylesheet: &'h CompiledStylesheet,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
    is_in_table_header: bool,
    variable_stack: Vec<HashMap<String, Value>>,
}

/// Helper to get the "children" of a JSON value.
/// For an object, children are its key-value pairs.
/// For an array, children are its elements.
/// The `Option<&str>` holds the key name for object children.
fn get_children(node: &Value) -> Vec<(&Value, Option<&str>)> {
    match node {
        Value::Object(map) => map.iter().map(|(k, v)| (v, Some(k.as_str()))).collect(),
        Value::Array(arr) => arr.iter().map(|v| (v, None)).collect(),
        _ => vec![],
    }
}

impl<'h> TemplateExecutor<'h> {
    pub fn new(
        template_engine: &'h Handlebars<'static>,
        stylesheet: &'h CompiledStylesheet,
    ) -> Self {
        Self {
            template_engine,
            stylesheet,
            node_stack: vec![],
            inline_stack: vec![],
            is_in_table_header: false,
            variable_stack: vec![HashMap::new()], // Start with a global scope
        }
    }

    /// The main public entry point for the executor.
    /// It starts processing from the root template (`match="/"`) against the initial data context.
    pub fn build_tree(&mut self, context: &Value) -> Result<Vec<IRNode>, ParseError> {
        self.node_stack.clear();
        self.inline_stack.clear();
        self.is_in_table_header = false;
        self.variable_stack = vec![HashMap::new()];

        let root_node = IRNode::Root(Vec::with_capacity(16));
        self.node_stack.push(root_node);

        // Find and execute the root template (`match="/"`).
        let root_template = self
            .stylesheet
            .root_template
            .as_ref()
            .ok_or_else(|| ParseError::TemplateParse("Missing root template (`match=\"/\")".to_string()))?;
        self.execute_template(root_template, context)?;

        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(ParseError::TemplateParse(
                "Failed to construct root node.".to_string(),
            ))
        }
    }

    fn get_current_variables(&self) -> &HashMap<String, Value> {
        self.variable_stack.last().unwrap()
    }

    /// Recursively walks the AST and builds the `IRNode` tree.
    fn execute_template(
        &mut self,
        template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<(), ParseError> {
        for instruction in &template.0 {
            if matches!(instruction, XsltInstruction::Text(s) if s.is_empty()) {
                continue;
            }
            match instruction {
                XsltInstruction::ForEach { select, body } => {
                    let selected_values = select.select(context, self.get_current_variables());
                    let items: Vec<Value> = if let Some(arr) =
                        selected_values.first().and_then(|v| v.as_array())
                    {
                        arr.clone()
                    } else {
                        selected_values.into_iter().cloned().collect()
                    };
                    for item_context in &items {
                        self.execute_template(body, item_context)?;
                    }
                }
                XsltInstruction::If { test, body } => {
                    if test.evaluate(context) {
                        self.execute_template(body, context)?;
                    }
                }
                XsltInstruction::ContentTag { tag_name, styles, attrs, body } => {
                    self.execute_start_tag(tag_name, styles, attrs, context)?;
                    self.execute_template(body, context)?;
                    self.execute_end_tag(tag_name)?;
                }
                XsltInstruction::EmptyTag { tag_name, styles, attrs } => {
                    self.execute_empty_tag(tag_name, styles, attrs, context)?;
                }
                XsltInstruction::Text(text) => {
                    let rendered_text = if text.contains("{{") {
                        self.render_text(text, context)?
                    } else {
                        text.clone()
                    };
                    self.push_inline_to_parent(InlineNode::Text(rendered_text));
                }
                XsltInstruction::ValueOf { select } => {
                    let content =
                        xpath::select_as_string(select, context, self.get_current_variables());
                    if !content.is_empty() {
                        self.push_inline_to_parent(InlineNode::Text(content));
                    }
                }
                XsltInstruction::CallTemplate { name, params } => {
                    let target_template = self.stylesheet.named_templates.get(name).ok_or_else(
                        || {
                            ParseError::TemplateParse(format!(
                                "Called template '{}' not found in stylesheet.",
                                name
                            ))
                        },
                    )?;
                    let mut new_scope = HashMap::new();
                    for param in params {
                        let value_vec =
                            param.select.select(context, self.get_current_variables());
                        if let Some(val) = value_vec.first() {
                            new_scope.insert(param.name.clone(), (*val).clone());
                        }
                    }
                    self.variable_stack.push(new_scope);
                    self.execute_template(target_template, context)?;
                    self.variable_stack.pop();
                }
                XsltInstruction::Table { styles, columns, header, body } => {
                    self.execute_table(styles, columns, header.as_ref(), body, context)?;
                }
                XsltInstruction::ApplyTemplates { select, mode } => {
                    if let Some(sel) = select {
                        // The selection result creates temporary values, so we must clone them to own them
                        // for the duration of this call.
                        let selected_values = sel.select(context, self.get_current_variables());
                        let owned_nodes: Vec<Value> = selected_values.into_iter().cloned().collect();
                        let nodes_to_process: Vec<(&Value, Option<&str>)> =
                            owned_nodes.iter().map(|v| (v, None)).collect();
                        self.apply_templates_to_nodes(nodes_to_process, mode.as_deref())?;
                    } else {
                        // `get_children` returns references to data inside `context`, which is guaranteed to live long enough.
                        let nodes_to_process = get_children(context);
                        self.apply_templates_to_nodes(nodes_to_process, mode.as_deref())?;
                    }
                }
            }
        }
        Ok(())
    }

    /// The core of the "push" model. Finds the best matching template for a set of nodes
    /// and executes them, or applies the built-in default rules.
    fn apply_templates_to_nodes(
        &mut self,
        nodes: Vec<(&Value, Option<&str>)>,
        mode: Option<&str>,
    ) -> Result<(), ParseError> {
        for (node, name) in nodes {
            let template_rule = self.find_matching_template(node, name, mode);
            if let Some(rule) = template_rule {
                // A matching template was found, execute it with the matched node as the new context.
                self.execute_template(&rule.body, node)?;
            } else {
                // No matching template found, apply the built-in default rule.
                self.apply_builtin_template(node)?;
            }
        }
        Ok(())
    }

    /// Applies XSLT's built-in template rules.
    fn apply_builtin_template(&mut self, node: &Value) -> Result<(), ParseError> {
        match node {
            // For elements (Objects/Arrays), the default is to process their children.
            Value::Object(_) | Value::Array(_) => {
                let children = get_children(node);
                // The built-in rule always applies templates in the default mode.
                self.apply_templates_to_nodes(children, None)?;
            }
            // For text nodes, the default is to output their string value.
            Value::String(s) => {
                self.push_inline_to_parent(InlineNode::Text(s.clone()));
            }
            Value::Number(n) => {
                self.push_inline_to_parent(InlineNode::Text(n.to_string()));
            }
            Value::Bool(b) => {
                self.push_inline_to_parent(InlineNode::Text(b.to_string()));
            }
            Value::Null => { /* Do nothing */ }
        }
        Ok(())
    }

    /// Finds the highest-priority template rule that matches a given node.
    fn find_matching_template(
        &self,
        node: &Value,
        name: Option<&str>,
        mode: Option<&str>,
    ) -> Option<&'h TemplateRule> {
        // Look up the rules for the given mode (or the default mode if `None`).
        self.stylesheet
            .template_rules
            .get(&mode.map(String::from))
            .and_then(|rules| {
                // The rules are pre-sorted by priority, so the first match is the best one.
                rules.iter().find(|rule| xpath::matches(node, name, &rule.match_pattern))
            })
    }

    fn execute_start_tag(
        &mut self,
        tag_name: &[u8],
        styles: &PreparsedStyles,
        attrs: &HashMap<String, String>,
        context: &Value,
    ) -> Result<(), ParseError> {
        let style_sets = styles.style_sets.clone();
        let style_override = styles.style_override.clone();

        match String::from_utf8_lossy(tag_name).as_ref() {
            "fo:list-block" | "list" => self.node_stack.push(IRNode::List {
                style_sets,
                style_override,
                children: vec![],
            }),
            "fo:list-item" | "list-item" => self.node_stack.push(IRNode::ListItem {
                style_sets,
                style_override,
                children: vec![],
            }),
            "flex-container" => self.node_stack.push(IRNode::FlexContainer {
                style_sets,
                style_override,
                children: vec![],
            }),
            "fo:block" | "text" | "p" => self.node_stack.push(IRNode::Paragraph {
                style_sets,
                style_override,
                children: vec![],
            }),
            "fo:basic-link" | "link" => {
                let href_template = attrs.get("href").cloned().unwrap_or_default();
                let href = self.render_text(&href_template, context)?;
                self.inline_stack.push(InlineNode::Hyperlink {
                    href,
                    style_sets,
                    style_override,
                    children: vec![],
                });
            }
            "fo:inline" | "strong" | "b" | "em" | "i" | "span" => {
                self.inline_stack.push(InlineNode::StyledSpan {
                    style_sets,
                    style_override,
                    children: vec![],
                });
            }
            "fo:table-row" | "row" => {
                let new_row = TableRow { cells: Vec::with_capacity(8) };
                if let Some(Table { header, body, .. }) = self.node_stack.last_mut() {
                    if self.is_in_table_header {
                        if let Some(h) = header {
                            h.rows.push(new_row);
                        }
                    } else {
                        body.rows.push(new_row);
                    }
                }
            }
            "fo:table-cell" | "cell" => {
                let new_cell = TableCell {
                    style_sets,
                    style_override,
                    children: Vec::with_capacity(2),
                };
                if let Some(Table { header, body, .. }) = self.node_stack.last_mut() {
                    let row = if self.is_in_table_header {
                        header.as_mut().and_then(|h| h.rows.last_mut())
                    } else {
                        body.rows.last_mut()
                    };
                    if let Some(r) = row {
                        r.cells.push(new_cell);
                    }
                }
            }
            _ => self.node_stack.push(IRNode::Block {
                style_sets,
                style_override,
                children: vec![],
            }),
        }
        Ok(())
    }

    fn execute_end_tag(&mut self, tag_name: &[u8]) -> Result<(), ParseError> {
        match String::from_utf8_lossy(tag_name).as_ref() {
            "fo:basic-link" | "link" | "fo:inline" | "strong" | "b" | "em" | "i" | "span" => {
                if let Some(node) = self.inline_stack.pop() {
                    self.push_inline_to_parent(node);
                }
            }
            "fo:table-row" | "row" | "fo:table-cell" | "cell" => { /* No op */ }
            _ => {
                if let Some(node) = self.node_stack.pop() {
                    self.push_block_to_parent(node);
                }
            }
        }
        Ok(())
    }

    fn execute_empty_tag(
        &mut self,
        tag_name: &[u8],
        styles: &PreparsedStyles,
        attrs: &HashMap<String, String>,
        context: &Value,
    ) -> Result<(), ParseError> {
        match String::from_utf8_lossy(tag_name).as_ref() {
            "fo:external-graphic" | "image" => {
                let src_template = attrs.get("src").cloned().unwrap_or_default();
                let src = self.render_text(&src_template, context)?;
                let style_sets = styles.style_sets.clone();
                let style_override = styles.style_override.clone();

                if matches!(self.node_stack.last(), Some(IRNode::Paragraph { .. }))
                    || !self.inline_stack.is_empty()
                {
                    self.push_inline_to_parent(InlineNode::Image {
                        src,
                        style_sets,
                        style_override,
                    });
                } else {
                    self.push_block_to_parent(IRNode::Image {
                        src,
                        style_sets,
                        style_override,
                    });
                }
            }
            "fo:block" | "br" => self.push_inline_to_parent(InlineNode::LineBreak),
            _ => {}
        }
        Ok(())
    }

    fn execute_table(
        &mut self,
        styles: &PreparsedStyles,
        columns: &[TableColumnDefinition],
        header_template: Option<&PreparsedTemplate>,
        body_template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<(), ParseError> {
        let table_node = Table {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            columns: columns
                .iter()
                .map(|c| idf::TableColumnDefinition {
                    width: c.width.clone(),
                    style: c.style.clone(),
                    header_style: c.header_style.clone(),
                })
                .collect(),
            calculated_widths: Vec::new(),
            header: if header_template.is_some() {
                Some(Box::new(TableHeader { rows: vec![] }))
            } else {
                None
            },
            body: Box::new(TableBody { rows: vec![] }),
        };
        self.node_stack.push(table_node);

        if let Some(template) = header_template {
            self.is_in_table_header = true;
            self.execute_template(template, context)?;
            self.is_in_table_header = false;
        }

        self.execute_template(body_template, context)?;

        if let Some(node) = self.node_stack.pop() {
            self.push_block_to_parent(node);
        }
        Ok(())
    }

    fn render_text(&self, text: &str, context: &Value) -> Result<String, ParseError> {
        self.template_engine
            .render_template(text, context)
            .map_err(|e| ParseError::TemplateRender(e.to_string()))
    }

    fn push_block_to_parent(&mut self, node: IRNode) {
        match self.node_stack.last_mut() {
            Some(IRNode::Root(children))
            | Some(IRNode::Block { children, .. })
            | Some(IRNode::FlexContainer { children, .. })
            | Some(IRNode::List { children, .. })
            | Some(IRNode::ListItem { children, .. }) => children.push(node),
            Some(IRNode::Table { .. }) => {
                let row = if self.is_in_table_header {
                    if let Some(IRNode::Table { header: Some(h), .. }) = self.node_stack.last_mut()
                    {
                        h.rows.last_mut()
                    } else {
                        None
                    }
                } else if let Some(IRNode::Table { body, .. }) = self.node_stack.last_mut() {
                    body.rows.last_mut()
                } else {
                    None
                };
                if let Some(r) = row {
                    if let Some(cell) = r.cells.last_mut() {
                        cell.children.push(node);
                    }
                }
            }
            _ => log::warn!("Cannot add block node to current parent."),
        }
    }

    fn push_inline_to_parent(&mut self, node: InlineNode) {
        match self.inline_stack.last_mut() {
            Some(InlineNode::StyledSpan { children, .. })
            | Some(InlineNode::Hyperlink { children, .. }) => children.push(node),
            _ => {
                if let Some(IRNode::Paragraph { children, .. }) = self.node_stack.last_mut() {
                    children.push(node);
                } else if let Some(IRNode::Table { .. }) = self.node_stack.last_mut() {
                    self.node_stack.push(IRNode::Paragraph {
                        style_sets: vec![],
                        style_override: None,
                        children: vec![node],
                    });
                    let p_node = self.node_stack.pop().unwrap();
                    self.push_block_to_parent(p_node);
                }
            }
        }
    }
}