// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/executor.rs
use super::ast::{
    CompiledStylesheet, PreparsedStyles, PreparsedTemplate, TemplateRule, XsltInstruction,
};
use crate::core::idf::{IRNode, InlineNode};
use crate::parser::datasource::{DataSourceNode, NodeType};
use crate::parser::xpath::{self, engine, functions::FunctionRegistry, XPathValue};
use crate::parser::ParseError;
use std::collections::HashMap;
use std::marker::PhantomData;

/// A stateful executor that constructs an `IRNode` tree by processing a `CompiledStylesheet`
/// against a generic `DataSourceNode`. It implements the XSLT "push" model.
pub struct TemplateExecutor<'s, 'a, N: DataSourceNode<'a>> {
    stylesheet: &'s CompiledStylesheet,
    functions: FunctionRegistry,
    root_node: N,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
    variable_stack: Vec<HashMap<String, XPathValue<N>>>,
    _marker: PhantomData<&'a ()>,
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor<'s, 'a, N> {
    pub fn new(stylesheet: &'s CompiledStylesheet, root_node: N) -> Self {
        Self {
            stylesheet,
            functions: FunctionRegistry::default(),
            root_node,
            node_stack: vec![],
            inline_stack: vec![],
            variable_stack: vec![HashMap::new()], // Start with a global scope
            _marker: PhantomData,
        }
    }

    /// The main public entry point for the executor.
    pub fn build_tree(&mut self) -> Result<Vec<IRNode>, ParseError> {
        self.node_stack.clear();
        self.inline_stack.clear();
        self.node_stack.push(IRNode::Root(Vec::with_capacity(16)));
        self.apply_templates_to_nodes(&[self.root_node], None)?;
        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(ParseError::TemplateParse("Failed to construct root node.".to_string()))
        }
    }

    // --- Scope Management ---
    fn push_scope(&mut self) {
        self.variable_stack.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.variable_stack.pop();
    }

    fn set_variable_in_current_scope(&mut self, name: String, value: XPathValue<N>) {
        if let Some(scope) = self.variable_stack.last_mut() {
            scope.insert(name, value);
        }
    }

    fn get_merged_variables(&self) -> HashMap<String, XPathValue<N>> {
        let mut merged = HashMap::new();
        for scope in &self.variable_stack {
            merged.extend(scope.clone());
        }
        merged
    }

    fn get_eval_context<'d>(
        &'d self,
        context_node: N,
        merged_variables: &'d HashMap<String, XPathValue<N>>,
        context_position: usize,
        context_size: usize,
    ) -> engine::EvaluationContext<'a, 'd, N> {
        engine::EvaluationContext::new(
            context_node,
            self.root_node,
            &self.functions,
            context_position,
            context_size,
            merged_variables,
        )
    }

    /// Processes a list of instructions from a template body against a context node.
    fn execute_template(
        &mut self,
        template: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<(), ParseError> {
        for instruction in &template.0 {
            self.execute_instruction(instruction, context_node, context_position, context_size)?;
        }
        Ok(())
    }

    /// Processes a single XSLT instruction.
    fn execute_instruction(
        &mut self,
        instruction: &XsltInstruction,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<(), ParseError> {
        match instruction {
            XsltInstruction::Text(text) => {
                self.push_inline_to_parent(InlineNode::Text(text.clone()));
            }
            XsltInstruction::ValueOf { select } => {
                let merged_vars = self.get_merged_variables();
                let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                let result = xpath::evaluate(select, &e_ctx)?;
                let content = result.to_string();
                if !content.is_empty() {
                    self.push_inline_to_parent(InlineNode::Text(content));
                }
            }
            XsltInstruction::Variable { name, select } => {
                let merged_vars = self.get_merged_variables();
                let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                let value = xpath::evaluate(select, &e_ctx)?;
                self.set_variable_in_current_scope(name.clone(), value);
            }
            XsltInstruction::ApplyTemplates { select, mode } => {
                let nodes_to_process = if let Some(sel) = select {
                    let merged_vars = self.get_merged_variables();
                    let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                    if let XPathValue::NodeSet(nodes) = xpath::evaluate(sel, &e_ctx)? {
                        nodes
                    } else {
                        vec![]
                    }
                } else {
                    context_node.children().collect()
                };
                self.apply_templates_to_nodes(&nodes_to_process, mode.as_deref())?;
            }
            XsltInstruction::If { test, body } => {
                let merged_vars = self.get_merged_variables();
                let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                if xpath::evaluate(test, &e_ctx)?.to_bool() {
                    self.execute_template(body, context_node, context_position, context_size)?;
                }
            }
            XsltInstruction::ForEach { select, body } => {
                let merged_vars = self.get_merged_variables();
                let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                if let XPathValue::NodeSet(nodes) = xpath::evaluate(select, &e_ctx)? {
                    let inner_context_size = nodes.len();
                    for (i, node) in nodes.into_iter().enumerate() {
                        let inner_context_position = i + 1;
                        self.push_scope();
                        self.execute_template(body, node, inner_context_position, inner_context_size)?;
                        self.pop_scope();
                    }
                }
            }
            XsltInstruction::CallTemplate { name, params } => {
                if let Some(template) = self.stylesheet.named_templates.get(name) {
                    let template_clone = template.clone();
                    let (passed_params, caller_merged_vars) = {
                        let merged_vars = self.get_merged_variables();
                        let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                        let params_map = params.iter()
                            .map(|param| Ok((param.name.clone(), xpath::evaluate(&param.select, &e_ctx)?)))
                            .collect::<Result<HashMap<_,_>, ParseError>>()?;
                        (params_map, merged_vars)
                    };
                    self.push_scope();
                    for defined_param in &template_clone.params {
                        let param_value = if let Some(passed_value) = passed_params.get(&defined_param.name) {
                            passed_value.clone()
                        } else if let Some(default_expr) = &defined_param.default_value {
                            let e_ctx = self.get_eval_context(context_node, &caller_merged_vars, context_position, context_size);
                            xpath::evaluate(default_expr, &e_ctx)?
                        } else {
                            XPathValue::String("".to_string())
                        };
                        self.set_variable_in_current_scope(defined_param.name.clone(), param_value);
                    }
                    self.execute_template(&template_clone.body, context_node, context_position, context_size)?;
                    self.pop_scope();
                } else {
                    return Err(ParseError::TemplateRender(format!("Call to unknown named template: '{}'", name)));
                }
            }
            XsltInstruction::ContentTag { tag_name, styles, attrs: _, body } => {
                self.execute_start_tag(tag_name, styles)?;
                self.execute_template(body, context_node, context_position, context_size)?;
                self.execute_end_tag(tag_name)?;
            }
            XsltInstruction::EmptyTag { .. } => {}
            _ => log::warn!("XSLT instruction not yet implemented: {:?}", instruction),
        }
        Ok(())
    }

    fn apply_templates_to_nodes(&mut self, nodes: &[N], mode: Option<&str>) -> Result<(), ParseError> {
        let context_size = nodes.len();
        for (i, &node) in nodes.iter().enumerate() {
            let context_position = i + 1;
            if let Some(rule) = self.find_matching_template(node, mode) {
                let body = rule.body.clone();
                self.push_scope();
                self.execute_template(&body, node, context_position, context_size)?;
                self.pop_scope();
            } else {
                self.apply_builtin_template(node)?;
            }
        }
        Ok(())
    }

    fn apply_builtin_template(&mut self, node: N) -> Result<(), ParseError> {
        match node.node_type() {
            NodeType::Root | NodeType::Element => {
                let children: Vec<N> = node.children().collect();
                self.apply_templates_to_nodes(&children, None)?;
            }
            NodeType::Text => {
                self.push_inline_to_parent(InlineNode::Text(node.string_value()));
            }
            NodeType::Attribute => {
                self.push_inline_to_parent(InlineNode::Text(node.string_value()));
            }
        }
        Ok(())
    }

    fn find_matching_template(&self, node: N, mode: Option<&str>) -> Option<&'s TemplateRule> {
        let rules_for_mode = self.stylesheet.template_rules.get(&mode.map(String::from))?;
        rules_for_mode.iter().find(|rule| rule.pattern.matches(node, self.root_node))
    }

    fn execute_start_tag(&mut self, tag_name: &[u8], styles: &PreparsedStyles) -> Result<(), ParseError> {
        let style_sets = styles.style_sets.clone(); let style_override = styles.style_override.clone();
        let node = match String::from_utf8_lossy(tag_name).as_ref() {
            "p" => IRNode::Paragraph { style_sets, style_override, children: vec![] },
            "fo:block" | "block" => IRNode::Block { style_sets, style_override, children: vec![] },
            "fo:flex-container" | "flex-container" => IRNode::FlexContainer { style_sets, style_override, children: vec![] },
            "fo:list-block" | "list" => IRNode::List { style_sets, style_override, start: None, children: vec![] },
            "fo:list-item" | "list-item" => IRNode::ListItem { style_sets, style_override, children: vec![] },
            "fo:inline" | "span" | "strong" | "b" | "em" | "i" => { self.inline_stack.push(InlineNode::StyledSpan { style_sets, style_override, children: vec![] }); return Ok(()); }
            "fo:basic-link" | "link" => { self.inline_stack.push(InlineNode::Hyperlink { style_sets, style_override, href: "".to_string(), children: vec![] }); return Ok(()); }
            _ => IRNode::Block { style_sets, style_override, children: vec![] },
        };
        self.node_stack.push(node); Ok(())
    }
    fn execute_end_tag(&mut self, tag_name: &[u8]) -> Result<(), ParseError> {
        match String::from_utf8_lossy(tag_name).as_ref() {
            "fo:inline" | "span" | "strong" | "b" | "em" | "i" | "fo:basic-link" | "link" => { if let Some(inline_node) = self.inline_stack.pop() { self.push_inline_to_parent(inline_node); } }
            _ => { if let Some(node) = self.node_stack.pop() { self.push_block_to_parent(node); } }
        }
        Ok(())
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
        if let Some(IRNode::Paragraph { children: c, .. }) = self.node_stack.last_mut() { c.push(node); }
        else { self.push_block_to_parent(IRNode::Paragraph { style_sets: vec![], style_override: None, children: vec![node], }); }
    }
}