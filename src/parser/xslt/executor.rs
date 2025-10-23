// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/executor.rs
// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/executor.rs
//! The stateful executor for an XSLT program. It orchestrates the execution flow,
//! manages state (like variables), and delegates output generation to an `OutputBuilder`.

use super::ast::{
    AttributeValueTemplate, AvtPart, CompiledStylesheet, PreparsedStyles, PreparsedTemplate,
    SortDataType, SortKey, SortOrder, TemplateRule, XsltInstruction,
};
use super::executor_handlers;
use super::idf_builder::IdfBuilder;
use super::output::OutputBuilder;
use crate::parser::xslt::datasource::{DataSourceNode, NodeType};
use crate::parser::xslt::xpath::{self, engine, functions::FunctionRegistry, XPathValue};
use crate::parser::ParseError;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;

// --- Execution Error Handling ---

#[derive(Debug)]
pub enum ExecutionError {
    XPath(String),
    UnknownNamedTemplate(String),
    FunctionError {
        function: String,
        message: String,
    },
    TypeError(String),
    // For errors from parsing AVTs etc. at runtime.
    Parse(ParseError),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::XPath(msg) => write!(f, "XPath evaluation failed: {}", msg),
            ExecutionError::UnknownNamedTemplate(name) => write!(f, "Call to unknown named template: '{}'", name),
            ExecutionError::FunctionError { function, message } => write!(f, "Error in function '{}': {}", function, message),
            ExecutionError::TypeError(msg) => write!(f, "Type error: {}", msg),
            ExecutionError::Parse(e) => write!(f, "Parse error during execution: {}", e),
        }
    }
}
impl std::error::Error for ExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let ExecutionError::Parse(e) = self { Some(e) } else { None }
    }
}
impl From<ParseError> for ExecutionError {
    fn from(e: ParseError) -> Self {
        ExecutionError::Parse(e)
    }
}


/// An index for a single `<xsl:key>`, mapping string values to lists of nodes.
type KeyIndex<'a, N> = HashMap<String, Vec<N>>;

/// A map of all defined keys, indexed by the key's name.
type KeyIndexMap<'a, N> = HashMap<String, KeyIndex<'a, N>>;

/// A stateful executor that constructs an `IRNode` tree by processing a `CompiledStylesheet`
/// against a generic `DataSourceNode`. It implements the XSLT "push" model.
pub struct TemplateExecutor<'s, 'a, N: DataSourceNode<'a>> {
    pub(crate) stylesheet: &'s CompiledStylesheet,
    pub(crate) functions: FunctionRegistry,
    pub(crate) root_node: N,
    pub(crate) variable_stack: Vec<HashMap<String, XPathValue<N>>>,
    /// Holds the pre-computed key indexes for the entire document.
    pub(crate) key_indexes: KeyIndexMap<'a, N>,
    /// If true, enables strict XSLT compliance checks.
    pub(crate) strict: bool,
    _marker: PhantomData<&'a ()>,
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor<'s, 'a, N> {
    pub fn new(stylesheet: &'s CompiledStylesheet, root_node: N, strict: bool) -> Result<Self, ParseError> {
        let functions = FunctionRegistry::default();
        let global_vars = HashMap::new();
        let key_indexes =
            Self::build_key_indexes(stylesheet, root_node, &functions, &global_vars)?;

        Ok(Self {
            stylesheet,
            functions,
            root_node,
            variable_stack: vec![global_vars], // Start with a global scope
            key_indexes,
            strict,
            _marker: PhantomData,
        })
    }

    /// Builds all key indexes for the document in a single pass.
    fn build_key_indexes(
        stylesheet: &'s CompiledStylesheet,
        root_node: N,
        functions: &FunctionRegistry,
        global_vars: &HashMap<String, XPathValue<N>>,
    ) -> Result<KeyIndexMap<'a, N>, ParseError> {
        let mut key_indexes: KeyIndexMap<'a, N> = HashMap::new();
        if stylesheet.keys.is_empty() {
            return Ok(key_indexes);
        }

        let empty_key_map = HashMap::new(); // For the e_ctx, since key() can't be used during indexing.

        // Single document traversal using an explicit stack (iterative DFS).
        let mut stack = vec![root_node];
        let mut visited = HashSet::new();
        visited.insert(root_node);

        while let Some(node) = stack.pop() {
            // For each node popped from the stack, check it against all key definitions.
            for key_def in &stylesheet.keys {
                if key_def.pattern.matches(node, root_node) {
                    // Node matches a key's pattern. Now evaluate the 'use' expression.
                    let e_ctx = engine::EvaluationContext::new(
                        node,
                        root_node,
                        functions,
                        1, 1, // Position/size are not well-defined, but 'use' rarely needs them.
                        global_vars,
                        &empty_key_map, // Pass empty map; key() not allowed in 'use' expr.
                        false, // Never use strict mode for key indexing
                    );
                    let use_result = xpath::evaluate(&key_def.use_expr, &e_ctx)
                        .map_err(|e| ParseError::TemplateRender(format!("Error evaluating 'use' expression for key '{}': {}", key_def.name, e)))?;

                    // The 'use' expression can return a node-set or a string/number.
                    let key_strings = match use_result {
                        XPathValue::NodeSet(nodes) => nodes
                            .into_iter()
                            .map(|n| n.string_value())
                            .collect::<Vec<_>>(),
                        other => vec![other.to_string()],
                    };

                    let index_for_this_key = key_indexes.entry(key_def.name.clone()).or_default();

                    // Add the *matched node* to the index for each resulting key string.
                    for key_str in key_strings {
                        if !key_str.is_empty() {
                            index_for_this_key
                                .entry(key_str)
                                .or_default()
                                .push(node);
                        }
                    }
                }
            }

            // After processing the node, add its children and attributes to the stack for future processing.
            for child in node.children() {
                if visited.insert(child) {
                    stack.push(child);
                }
            }
            for attr in node.attributes() {
                if visited.insert(attr) {
                    stack.push(attr);
                }
            }
        }
        Ok(key_indexes)
    }

    /// The main public entry point for the executor. It creates a concrete `IdfBuilder`
    /// and executes the template to produce a final `IRNode` tree.
    pub fn build_tree(&mut self) -> Result<Vec<crate::core::idf::IRNode>, ExecutionError> {
        let mut builder = IdfBuilder::new();
        self.execute(&mut builder)?;
        Ok(builder.get_result())
    }

    /// Executes the entire XSLT transformation, delegating all output actions to the provided builder.
    pub fn execute(&mut self, builder: &mut dyn OutputBuilder) -> Result<(), ExecutionError> {
        self.apply_templates_to_nodes(&[self.root_node], None, builder)?;
        Ok(())
    }

    // --- Scope Management ---
    pub(crate) fn push_scope(&mut self) {
        self.variable_stack.push(HashMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.variable_stack.pop();
    }

    pub(crate) fn set_variable_in_current_scope(&mut self, name: String, value: XPathValue<N>) {
        if let Some(scope) = self.variable_stack.last_mut() {
            scope.insert(name, value);
        }
    }

    pub(crate) fn get_merged_variables(&self) -> HashMap<String, XPathValue<N>> {
        let mut merged = HashMap::new();
        for scope in &self.variable_stack {
            merged.extend(scope.clone());
        }
        merged
    }

    pub(crate) fn get_eval_context<'d>(
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
            &self.key_indexes,
            self.strict,
        )
    }

    /// Processes a list of instructions from a template body against a context node.
    pub(crate) fn execute_template(
        &mut self,
        template: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        for instruction in &template.0 {
            self.execute_instruction(instruction, context_node, context_position, context_size, builder)?;
        }
        Ok(())
    }

    /// Evaluates an AVT and returns the resulting string.
    pub(crate) fn evaluate_avt(
        &self,
        avt: &AttributeValueTemplate,
        e_ctx: &engine::EvaluationContext<'a, '_, N>,
    ) -> Result<String, ExecutionError> {
        match avt {
            AttributeValueTemplate::Static(s) => Ok(s.clone()),
            AttributeValueTemplate::Dynamic(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        AvtPart::Static(s) => result.push_str(s),
                        AvtPart::Dynamic(expression) => {
                            let s = xpath::evaluate(expression, e_ctx)?.to_string();
                            result.push_str(&s);
                        }
                    }
                }
                Ok(result)
            }
        }
    }

    /// Processes a single XSLT instruction by dispatching to the appropriate handler.
    fn execute_instruction(
        &mut self,
        instruction: &XsltInstruction,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        match instruction {
            XsltInstruction::Text(text) => executor_handlers::literals::handle_text(text, builder),
            XsltInstruction::ValueOf { select } => {
                let merged_vars = self.get_merged_variables();
                let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                executor_handlers::literals::handle_value_of(select, &e_ctx, builder)?
            }
            XsltInstruction::CopyOf { select } => {
                let result = {
                    let merged_vars = self.get_merged_variables();
                    let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                    xpath::evaluate(select, &e_ctx)?
                };
                executor_handlers::copy::handle_copy_of(self, result, builder)?
            }
            XsltInstruction::Copy { styles, body } => executor_handlers::copy::handle_copy(self, styles, body, context_node, context_position, context_size, builder)?,
            XsltInstruction::Variable { name, select } => {
                let value = {
                    let merged_vars = self.get_merged_variables();
                    let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                    xpath::evaluate(select, &e_ctx)?
                };
                executor_handlers::variables::handle_variable(self, name, value)?
            }
            XsltInstruction::ApplyTemplates { select, mode, sort_keys } => {
                executor_handlers::apply_templates::handle_apply_templates(self, select, mode, sort_keys, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::If { test, body } => {
                let condition = {
                    let merged_vars = self.get_merged_variables();
                    let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                    xpath::evaluate(test, &e_ctx)?.to_bool()
                };
                executor_handlers::control_flow::handle_if(self, condition, body, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::Choose { whens, otherwise } => {
                executor_handlers::control_flow::handle_choose(self, whens, otherwise, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::ForEach { select, sort_keys, body } => {
                executor_handlers::for_each::handle_for_each(self, select, sort_keys, body, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::CallTemplate { name, params } => {
                executor_handlers::call_template::handle_call_template(self, name, params, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::ContentTag { tag_name, styles, attrs, body } => {
                let evaluated_attrs = {
                    let merged_vars = self.get_merged_variables();
                    let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                    attrs
                        .iter()
                        .map(|(name, avt)| Ok((name.clone(), self.evaluate_avt(avt, &e_ctx)?)))
                        .collect::<Result<HashMap<_, _>, ExecutionError>>()?
                };
                executor_handlers::literals::handle_content_tag(self, tag_name, styles, &evaluated_attrs, body, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::EmptyTag { tag_name, styles, attrs } => {
                let evaluated_attrs = {
                    let merged_vars = self.get_merged_variables();
                    let e_ctx = self.get_eval_context(context_node, &merged_vars, context_position, context_size);
                    attrs
                        .iter()
                        .map(|(name, avt)| Ok((name.clone(), self.evaluate_avt(avt, &e_ctx)?)))
                        .collect::<Result<HashMap<_, _>, ExecutionError>>()?
                };
                executor_handlers::literals::handle_empty_tag(self, tag_name, styles, &evaluated_attrs, builder)?
            }
            XsltInstruction::Attribute { name, body } => {
                executor_handlers::literals::handle_attribute(self, name, body, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::Element { name, body } => {
                executor_handlers::literals::handle_element(self, name, body, context_node, context_position, context_size, builder)?
            }
            XsltInstruction::Table { styles, columns, header, body } => {
                executor_handlers::table::handle_table(self, styles, columns, header, body, context_node, context_position, context_size, builder)?
            }
            _ => log::warn!("XSLT instruction not yet implemented: {:?}", instruction),
        }
        Ok(())
    }

    pub(crate) fn apply_templates_to_nodes(&mut self, nodes: &[N], mode: Option<&str>, builder: &mut dyn OutputBuilder) -> Result<(), ExecutionError> {
        let context_size = nodes.len();
        for (i, &node) in nodes.iter().enumerate() {
            let context_position = i + 1;
            if let Some(rule) = self.find_matching_template(node, mode) {
                let body = rule.body.clone();
                self.push_scope();
                self.execute_template(&body, node, context_position, context_size, builder)?;
                self.pop_scope();
            } else {
                self.apply_builtin_template(node, builder)?;
            }
        }
        Ok(())
    }

    fn apply_builtin_template(&mut self, node: N, builder: &mut dyn OutputBuilder) -> Result<(), ExecutionError> {
        match node.node_type() {
            NodeType::Root | NodeType::Element => {
                let children: Vec<N> = node.children().collect();
                self.apply_templates_to_nodes(&children, None, builder)?;
            }
            NodeType::Text | NodeType::Attribute => {
                builder.add_text(&node.string_value());
            }
            NodeType::Comment | NodeType::ProcessingInstruction => {
                // The built-in template rules for comments and processing instructions do nothing.
            }
        }
        Ok(())
    }

    fn find_matching_template(&self, node: N, mode: Option<&str>) -> Option<&'s TemplateRule> {
        let rules_for_mode = self.stylesheet.template_rules.get(&mode.map(String::from))?;
        rules_for_mode.iter().find(|rule| rule.pattern.matches(node, self.root_node))
    }

    pub(crate) fn sort_node_set(
        &self,
        nodes: &mut [N],
        sort_keys: &[SortKey],
        merged_vars: &HashMap<String, XPathValue<N>>,
    ) -> Result<(), ExecutionError> {
        if sort_keys.is_empty() {
            return Ok(());
        }

        let mut sort_results = HashMap::new();
        for (i, &node) in nodes.iter().enumerate() {
            for (key_idx, key) in sort_keys.iter().enumerate() {
                let e_ctx = self.get_eval_context(node, merged_vars, i + 1, nodes.len());
                let value = xpath::evaluate(&key.select, &e_ctx)?;
                sort_results.insert((node, key_idx), value);
            }
        }

        nodes.sort_by(|&a, &b| {
            for (key_idx, key) in sort_keys.iter().enumerate() {
                let val_a = sort_results.get(&(a, key_idx)).unwrap();
                let val_b = sort_results.get(&(b, key_idx)).unwrap();

                let ordering = match key.data_type {
                    SortDataType::Number => {
                        let num_a = val_a.to_number();
                        let num_b = val_b.to_number();
                        num_a.partial_cmp(&num_b).unwrap_or(Ordering::Equal)
                    }
                    SortDataType::Text => {
                        val_a.to_string().cmp(&val_b.to_string())
                    }
                };
                let final_ordering = if key.order == SortOrder::Descending {
                    ordering.reverse()
                } else {
                    ordering
                };

                if final_ordering != Ordering::Equal {
                    return final_ordering;
                }
            }
            Ordering::Equal
        });

        Ok(())
    }

    /// Maps a literal result element tag name to a semantic action on the `OutputBuilder`.
    pub(crate) fn execute_start_tag(&self, tag_name: &[u8], styles: &PreparsedStyles, builder: &mut dyn OutputBuilder) {
        match String::from_utf8_lossy(tag_name).as_ref() {
            "p" => builder.start_paragraph(styles),
            "fo:block" | "block" | "root" | "div" => builder.start_block(styles),
            "fo:flex-container" | "flex-container" => builder.start_flex_container(styles),
            "fo:list-block" | "list" => builder.start_list(styles),
            "fo:list-item" | "list-item" => builder.start_list_item(styles),
            "fo:inline" | "span" | "strong" | "b" | "em" | "i" => builder.start_styled_span(styles),
            "fo:basic-link" | "a" | "link" => builder.start_hyperlink(styles),
            "fo:external-graphic" | "img" => builder.start_image(styles),
            // Table elements
            "table" | "fo:table" => builder.start_table(styles),
            "tbody" | "thead" | "header" => builder.start_block(styles), // Treat as simple containers
            "tr" | "row" | "fo:table-row" => builder.start_table_row(styles),
            "td" | "cell" | "fo:table-cell" => builder.start_table_cell(styles),
            _ => builder.start_block(styles), // Default to a block for unknown tags
        };
    }

    /// Maps a literal result element tag name to a semantic end action on the `OutputBuilder`.
    pub(crate) fn execute_end_tag(&self, tag_name: &[u8], builder: &mut dyn OutputBuilder) {
        match String::from_utf8_lossy(tag_name).as_ref() {
            "p" => builder.end_paragraph(),
            "fo:block" | "block" | "root" | "div" => builder.end_block(),
            "fo:flex-container" | "flex-container" => builder.end_flex_container(),
            "fo:list-block" | "list" => builder.end_list(),
            "fo:list-item" | "list-item" => builder.end_list_item(),
            "fo:inline" | "span" | "strong" | "b" | "em" | "i" => builder.end_styled_span(),
            "fo:basic-link" | "a" | "link" => builder.end_hyperlink(),
            "fo:external-graphic" | "img" => builder.end_image(),
            // Table elements
            "table" | "fo:table" => builder.end_table(),
            "tbody" | "thead" | "header" => builder.end_block(),
            "tr" | "row" | "fo:table-row" => builder.end_table_row(),
            "td" | "cell" | "fo:table-cell" => builder.end_table_cell(),
            _ => builder.end_block(),
        }
    }
}