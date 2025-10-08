// FILE: src/parser/xslt/executor.rs
use super::ast::{
    CompiledStylesheet, PreparsedStyles, PreparsedTemplate, TemplateRule, XsltInstruction,
};
use crate::core::idf::{self, IRNode, InlineNode, TableBody, TableCell, TableHeader, TableRow};
use crate::parser::ParseError;
use crate::xpath::{self, engine, functions::FunctionRegistry};
use serde_json::Value;
use std::collections::HashMap;

/// A stateful executor that constructs an `IRNode` tree by processing a `CompiledStylesheet`
/// against a JSON data context. It implements the XSLT "push" model.
pub struct TemplateExecutor<'s> {
    stylesheet: &'s CompiledStylesheet,
    functions: FunctionRegistry,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
    is_in_table_header: bool,
    variable_stack: Vec<HashMap<String, Value>>,
    current_loop_pos: Option<usize>,
}

/// Helper to get the "children" of a JSON value.
fn get_children(node: &Value) -> Vec<(&Value, Option<&str>)> {
    match node {
        Value::Object(map) => map.iter().map(|(k, v)| (v, Some(k.as_str()))).collect(),
        Value::Array(arr) => arr.iter().map(|v| (v, None)).collect(),
        _ => vec![],
    }
}

impl<'s> TemplateExecutor<'s> {
    pub fn new(stylesheet: &'s CompiledStylesheet) -> Self {
        Self {
            stylesheet,
            functions: FunctionRegistry::default(),
            node_stack: vec![],
            inline_stack: vec![],
            is_in_table_header: false,
            variable_stack: vec![HashMap::new()], // Start with a global scope
            current_loop_pos: None,
        }
    }

    /// The main public entry point for the executor.
    pub fn build_tree(&mut self, context: &Value) -> Result<Vec<IRNode>, ParseError> {
        self.node_stack.clear();
        self.inline_stack.clear();
        self.is_in_table_header = false;
        self.variable_stack = vec![HashMap::new()];
        self.current_loop_pos = None;

        self.node_stack.push(IRNode::Root(Vec::with_capacity(16)));

        let root_template = self.stylesheet.root_template.as_ref().ok_or_else(|| {
            ParseError::TemplateParse("Missing root template (`match=\"/\")".to_string())
        })?;
        self.execute_template(root_template, context)?;

        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(ParseError::TemplateParse("Failed to construct root node.".to_string()))
        }
    }

    fn get_current_variables(&self) -> &HashMap<String, Value> {
        self.variable_stack.last().unwrap()
    }

    fn get_eval_context<'a>(
        &'a self,
        context_node: &'a Value,
    ) -> engine::EvaluationContext<'a> {
        engine::EvaluationContext {
            context_node,
            variables: self.get_current_variables(),
            functions: &self.functions,
            loop_position: self.current_loop_pos,
        }
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
                    let e_ctx = self.get_eval_context(context);
                    let result_val = engine::evaluate(select, &e_ctx)?;

                    let mut nodes_to_iterate = Vec::new();
                    if let Some(arr) = result_val.as_array() {
                        nodes_to_iterate.extend(arr.iter());
                    } else if !result_val.is_null() {
                        nodes_to_iterate.push(&result_val);
                    }

                    let outer_loop_pos = self.current_loop_pos;
                    for (i, item_context) in nodes_to_iterate.into_iter().enumerate() {
                        self.current_loop_pos = Some(i);
                        self.execute_template(body, item_context)?;
                    }
                    self.current_loop_pos = outer_loop_pos;
                }
                XsltInstruction::If { test, body } => {
                    let e_ctx = self.get_eval_context(context);
                    if engine::evaluate_as_bool(test, &e_ctx)? {
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
                    let rendered_text = self.render_text(text, context)?;
                    self.push_inline_to_parent(InlineNode::Text(rendered_text));
                }
                XsltInstruction::ValueOf { select } => {
                    let e_ctx = self.get_eval_context(context);
                    let content = engine::evaluate_as_string(select, &e_ctx)?;
                    if !content.is_empty() {
                        self.push_inline_to_parent(InlineNode::Text(content));
                    }
                }
                XsltInstruction::CallTemplate { name, params } => {
                    let target_template = self.stylesheet.named_templates.get(name).ok_or_else(
                        || ParseError::TemplateParse(format!("Called template '{}' not found.", name)),
                    )?;
                    let mut new_scope = HashMap::new();
                    for param in params {
                        let e_ctx = self.get_eval_context(context);
                        let value = engine::evaluate(&param.select, &e_ctx)?;
                        new_scope.insert(param.name.clone(), value);
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
                        let e_ctx = self.get_eval_context(context);
                        let result_val = engine::evaluate(sel, &e_ctx)?;
                        let nodes_to_process = result_val
                            .as_array()
                            .map(|arr| arr.iter().map(|v| (v, None)).collect())
                            .unwrap_or_else(|| vec![(&result_val, None)]);
                        self.apply_templates_to_nodes(nodes_to_process, mode.as_deref())?;
                    } else {
                        let nodes_to_process = get_children(context);
                        self.apply_templates_to_nodes(nodes_to_process, mode.as_deref())?;
                    }
                }
                XsltInstruction::PageBreak { master_name } => {
                    self.push_block_to_parent(IRNode::PageBreak { master_name: master_name.clone() });
                }
            }
        }
        Ok(())
    }

    /// The core of the "push" model. Finds the best matching template for a set of nodes.
    fn apply_templates_to_nodes(
        &mut self,
        nodes: Vec<(&Value, Option<&str>)>,
        mode: Option<&str>,
    ) -> Result<(), ParseError> {
        let outer_loop_pos = self.current_loop_pos;
        for (i, &(node, name)) in nodes.iter().enumerate() {
            self.current_loop_pos = Some(i);
            if let Some(rule) = self.find_matching_template(node, name, mode) {
                self.execute_template(&rule.body, node)?;
            } else {
                self.apply_builtin_template(node)?;
            }
        }
        self.current_loop_pos = outer_loop_pos;
        Ok(())
    }

    /// Applies XSLT's built-in template rules.
    fn apply_builtin_template(&mut self, node: &Value) -> Result<(), ParseError> {
        match node {
            Value::Object(_) | Value::Array(_) => {
                self.apply_templates_to_nodes(get_children(node), None)?;
            }
            Value::String(s) => self.push_inline_to_parent(InlineNode::Text(s.clone())),
            Value::Number(n) => self.push_inline_to_parent(InlineNode::Text(n.to_string())),
            Value::Bool(b) => self.push_inline_to_parent(InlineNode::Text(b.to_string())),
            Value::Null => {}
        }
        Ok(())
    }

    /// Finds the highest-priority template rule that matches a given node.
    fn find_matching_template(
        &self,
        node: &Value,
        name: Option<&str>,
        mode: Option<&str>,
    ) -> Option<&'s TemplateRule> {
        self.stylesheet
            .template_rules
            .get(&mode.map(String::from))
            .and_then(|rules| {
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
            "fo:list-block" | "list" => self.node_stack.push(IRNode::List { style_sets, style_override, start: None, children: vec![] }),
            "fo:list-item" | "list-item" => self.node_stack.push(IRNode::ListItem { style_sets, style_override, children: vec![] }),
            "flex-container" => self.node_stack.push(IRNode::FlexContainer { style_sets, style_override, children: vec![] }),

            // ======================= START: THIS IS THE FIX =======================
            // Remove "fo:block" and "block" from this line. They will be handled by
            // the default case, which correctly creates an IRNode::Block.
            "text" | "p" => self.node_stack.push(IRNode::Paragraph { style_sets, style_override, children: vec![] }),
            // ======================== END: THIS IS THE FIX ========================

            "fo:basic-link" | "link" => {
                let href = self.render_text(attrs.get("href").unwrap_or(&String::new()), context)?;
                self.inline_stack.push(InlineNode::Hyperlink { href, style_sets, style_override, children: vec![] });
            }
            "fo:inline" | "strong" | "b" | "em" | "i" | "span" => {
                self.inline_stack.push(InlineNode::StyledSpan { style_sets, style_override, children: vec![] });
            }
            "fo:table-row" | "row" => {
                if let Some(IRNode::Table { header, body, .. }) = self.node_stack.last_mut() {
                    let target_rows = if self.is_in_table_header { header.as_mut().map(|h| &mut h.rows) } else { Some(&mut body.rows) };
                    if let Some(rows) = target_rows {
                        rows.push(TableRow { cells: vec![] });
                    }
                }
            }
            "fo:table-cell" | "cell" => {
                if let Some(IRNode::Table { header, body, .. }) = self.node_stack.last_mut() {
                    let target_row = if self.is_in_table_header { header.as_mut().and_then(|h| h.rows.last_mut()) } else { body.rows.last_mut() };
                    if let Some(row) = target_row {
                        row.cells.push(TableCell { style_sets, style_override, children: vec![], colspan: 1, rowspan: 1 });
                    }
                }
            }
            _ => self.node_stack.push(IRNode::Block { style_sets, style_override, children: vec![] }),
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
            "fo:table-row" | "row" | "fo:table-cell" | "cell" => {}
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
                let src = self.render_text(attrs.get("src").unwrap_or(&String::new()), context)?;
                let style_sets = styles.style_sets.clone();
                let style_override = styles.style_override.clone();
                let is_inline_context = !self.inline_stack.is_empty()
                    || matches!(self.node_stack.last(), Some(IRNode::Paragraph { .. }));

                if is_inline_context {
                    self.push_inline_to_parent(InlineNode::Image { src, style_sets, style_override });
                } else {
                    self.push_block_to_parent(IRNode::Image { src, style_sets, style_override });
                }
            }
            "br" => self.push_inline_to_parent(InlineNode::LineBreak),
            _ => {}
        }
        Ok(())
    }

    fn execute_table(
        &mut self,
        styles: &PreparsedStyles,
        columns: &[crate::core::style::dimension::Dimension],
        header_template: Option<&PreparsedTemplate>,
        body_template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<(), ParseError> {
        let table_node = IRNode::Table {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            columns: columns
                .iter()
                .map(|d| idf::TableColumnDefinition { width: Some(d.clone()), ..Default::default() })
                .collect(),
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
        if !text.contains('{') {
            return Ok(text.to_string());
        }

        let mut result = String::new();
        let mut last_end = 0;
        let mut search_from = 0;
        while let Some(start_offset) = text[search_from..].find('{') {
            let start = search_from + start_offset;

            if text.as_bytes().get(start + 1) == Some(&b'{') {
                result.push_str(&text[last_end..=start]);
                last_end = start + 2;
                search_from = start + 2;
                continue;
            }

            if start > last_end {
                result.push_str(&text[last_end..start]);
            }

            let end_offset = text[start..]
                .find('}')
                .ok_or_else(|| ParseError::TemplateParse("Unclosed '{' expression".to_string()))?;
            let end_abs = start + end_offset;

            let inner = &text[start + 1..end_abs];
            let expression = xpath::parse_expression(inner.trim())?;
            let e_ctx = self.get_eval_context(context);
            let s = engine::evaluate_as_string(&expression, &e_ctx)?;
            result.push_str(&s);

            last_end = end_abs + 1;
            search_from = end_abs + 1;
        }

        if last_end < text.len() {
            result.push_str(&text[last_end..]);
        }
        Ok(result)
    }

    fn push_block_to_parent(&mut self, node: IRNode) {
        let parent = self.node_stack.last_mut();
        match parent {
            Some(IRNode::Root(c)) | Some(IRNode::Block { children: c, .. }) | Some(IRNode::FlexContainer { children: c, .. }) | Some(IRNode::List { children: c, .. }) | Some(IRNode::ListItem { children: c, .. }) => c.push(node),
            Some(IRNode::Table { header, body, .. }) => {
                let row = if self.is_in_table_header { header.as_mut().and_then(|h| h.rows.last_mut()) } else { body.rows.last_mut() };
                if let Some(cell) = row.and_then(|r| r.cells.last_mut()) {
                    cell.children.push(node);
                }
            }
            _ => log::warn!("Cannot add block node to current parent."),
        }
    }

    fn push_inline_to_parent(&mut self, node: InlineNode) {
        if let Some(inline_parent) = self.inline_stack.last_mut() {
            if let InlineNode::StyledSpan { children: c, .. }
            | InlineNode::Hyperlink { children: c, .. } = inline_parent
            {
                c.push(node);
                return;
            }
        }
        if let Some(IRNode::Paragraph { children: c, .. }) = self.node_stack.last_mut() {
            c.push(node);
        } else {
            let paragraph =
                IRNode::Paragraph { style_sets: vec![], style_override: None, children: vec![node] };
            self.push_block_to_parent(paragraph);
        }
    }
}