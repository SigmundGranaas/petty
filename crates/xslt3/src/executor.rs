//! XSLT 3.0 template execution engine.
//!
//! This module provides [`TemplateExecutor3`], which interprets a [`CompiledStylesheet3`]
//! against input data to produce IR nodes for rendering.
//!
//! # Usage
//!
//! ```rust,ignore
//! let mut executor = TemplateExecutor3::new(&stylesheet, root_node, strict)?;
//! let ir_nodes = executor.build_tree()?;
//! ```
//!
//! # Execution Model
//!
//! The executor maintains:
//! - **Variable stack**: Scoped variables pushed/popped during template execution
//! - **Accumulator state**: Current values for streaming accumulators
//! - **Grouping context**: Current group and key for `xsl:for-each-group`
//! - **Mode state**: Active template mode for `xsl:apply-templates`
//!
//! # Error Handling
//!
//! Errors are returned as [`ExecutionError`] variants covering XPath evaluation,
//! type errors, dynamic errors with XSLT error codes, and control flow signals.

use crate::ast::{
    AccumulatorPhase, CompiledStylesheet3, OnNoMatch, PreparsedTemplate, TemplateRule3,
    TextValueTemplate, TvtPart, Xslt3Instruction,
};
use crate::error::Xslt3Error;
use crate::streaming::{parse_and_stream, parse_and_stream_with_accumulators};
use petty_idf::IRNode;
use petty_traits::ResourceProvider;
use petty_xpath1::XPathValue;
use petty_xpath1::datasource::{DataSourceNode, NodeType};
use petty_xpath31::types::{XdmItem, XdmValue};
use petty_xslt::ast::{AttributeValueTemplate, PreparsedStyles};
use petty_xslt::idf_builder::IdfBuilder;
use petty_xslt::output::{OutputBuilder, OutputSink};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ExecutionError {
    XPath(String),
    UnknownNamedTemplate(String),
    UnknownFunction(String),
    TypeError(String),
    AssertionFailed(String),
    DynamicError { code: String, message: String },
    Stream(String),
    Resource(String),
    Break,
    NextIteration(Vec<(String, String)>),
    NoMatchingTemplate { node_name: String },
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::XPath(msg) => write!(f, "XPath evaluation failed: {}", msg),
            ExecutionError::UnknownNamedTemplate(name) => {
                write!(f, "Call to unknown named template: '{}'", name)
            }
            ExecutionError::UnknownFunction(name) => {
                write!(f, "Call to unknown function: '{}'", name)
            }
            ExecutionError::TypeError(msg) => write!(f, "Type error: {}", msg),
            ExecutionError::AssertionFailed(msg) => write!(f, "Assertion failed: {}", msg),
            ExecutionError::DynamicError { code, message } => {
                write!(f, "Dynamic error [{}]: {}", code, message)
            }
            ExecutionError::Stream(msg) => write!(f, "Streaming error: {}", msg),
            ExecutionError::Resource(msg) => write!(f, "Resource error: {}", msg),
            ExecutionError::Break => write!(f, "Break signal"),
            ExecutionError::NextIteration(_) => write!(f, "Next iteration signal"),
            ExecutionError::NoMatchingTemplate { node_name } => {
                write!(
                    f,
                    "No matching template for node '{}' (on-no-match=fail)",
                    node_name
                )
            }
        }
    }
}

impl std::error::Error for ExecutionError {}

impl From<petty_xpath31::XPath31Error> for ExecutionError {
    fn from(e: petty_xpath31::XPath31Error) -> Self {
        ExecutionError::XPath(e.to_string())
    }
}

impl From<Xslt3Error> for ExecutionError {
    fn from(e: Xslt3Error) -> Self {
        ExecutionError::XPath(e.to_string())
    }
}

impl From<petty_xpath1::XPathError> for ExecutionError {
    fn from(e: petty_xpath1::XPathError) -> Self {
        ExecutionError::XPath(e.to_string())
    }
}

pub struct TemplateExecutor3<'s, 'a, N: DataSourceNode<'a>> {
    pub(crate) stylesheet: &'s CompiledStylesheet3,
    pub(crate) root_node: N,
    pub(crate) variable_stack: Vec<HashMap<String, XdmValue<N>>>,
    pub(crate) accumulator_values: HashMap<String, String>,
    pub(crate) accumulator_before_values: HashMap<String, String>,
    pub(crate) functions: petty_xpath1::functions::FunctionRegistry,
    pub(crate) strict: bool,
    pub(crate) current_grouping_key: Option<String>,
    pub(crate) current_group: Vec<N>,
    pub(crate) current_merge_key: Option<String>,
    pub(crate) current_merge_group: Vec<N>,
    pub(crate) current_merge_source: Option<String>,
    pub(crate) current_template_index: Option<usize>,
    pub(crate) current_mode: Option<String>,
    pub(crate) regex_match: Option<String>,
    pub(crate) regex_groups: Vec<String>,
    pub(crate) resource_provider: Option<Arc<dyn ResourceProvider>>,
    pub(crate) output_sink: Option<Arc<dyn OutputSink>>,
    pub(crate) active_result_documents: Vec<String>,
    pub(crate) last_constructed_value: Option<XdmValue<N>>,
    pub(crate) key_indexes: HashMap<String, HashMap<String, Vec<N>>>,
    _marker: PhantomData<&'a ()>,
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub fn new(
        stylesheet: &'s CompiledStylesheet3,
        root_node: N,
        strict: bool,
    ) -> Result<Self, Xslt3Error> {
        let global_vars = HashMap::new();
        let accumulator_values = HashMap::new();
        let accumulator_before_values = HashMap::new();

        let mut executor = Self {
            stylesheet,
            root_node,
            variable_stack: vec![global_vars],
            accumulator_values,
            accumulator_before_values,
            functions: petty_xpath1::functions::FunctionRegistry::default(),
            strict,
            current_grouping_key: None,
            current_group: Vec::new(),
            current_merge_key: None,
            current_merge_group: Vec::new(),
            current_merge_source: None,
            current_template_index: None,
            current_mode: None,
            regex_match: None,
            regex_groups: Vec::new(),
            resource_provider: None,
            output_sink: None,
            active_result_documents: Vec::new(),
            last_constructed_value: None,
            key_indexes: HashMap::new(),
            _marker: PhantomData,
        };

        executor.initialize_global_variables()?;
        executor
            .initialize_accumulators()
            .map_err(|e| Xslt3Error::runtime(e.to_string()))?;
        executor.build_key_indexes()?;

        Ok(executor)
    }

    fn build_key_indexes(&mut self) -> Result<(), Xslt3Error> {
        for (key_name, key_decl) in &self.stylesheet.keys {
            let mut index: HashMap<String, Vec<N>> = HashMap::new();

            self.build_key_index_recursive(
                self.root_node,
                &key_decl.match_pattern.clone(),
                &key_decl.use_expr.clone(),
                &mut index,
            )?;

            self.key_indexes.insert(key_name.clone(), index);
        }
        Ok(())
    }

    fn build_key_index_recursive(
        &self,
        node: N,
        match_pattern: &str,
        use_expr: &petty_xpath31::Expression,
        index: &mut HashMap<String, Vec<N>>,
    ) -> Result<(), Xslt3Error> {
        if self.node_matches_key_pattern(node, match_pattern) {
            let key_value = self
                .evaluate_xpath31(use_expr, node, 1, 1)
                .map_err(|e| Xslt3Error::runtime(e.to_string()))?;

            index.entry(key_value).or_default().push(node);
        }

        for child in node.children() {
            self.build_key_index_recursive(child, match_pattern, use_expr, index)?;
        }

        Ok(())
    }

    fn node_matches_key_pattern(&self, node: N, pattern: &str) -> bool {
        let pattern = pattern.trim();

        if pattern == "*" {
            return node.node_type() == petty_xpath1::datasource::NodeType::Element;
        }

        if let Some(name) = node.name() {
            name.local_part == pattern
        } else {
            false
        }
    }

    /// Set the resource provider for loading external documents (xsl:stream, xsl:source-document).
    pub fn with_resource_provider(mut self, provider: Arc<dyn ResourceProvider>) -> Self {
        self.resource_provider = Some(provider);
        self
    }

    /// Set the resource provider (mutable version).
    pub fn set_resource_provider(&mut self, provider: Arc<dyn ResourceProvider>) {
        self.resource_provider = Some(provider);
    }

    /// Set the output sink for multi-document output (xsl:result-document).
    pub fn with_output_sink(mut self, sink: Arc<dyn OutputSink>) -> Self {
        self.output_sink = Some(sink);
        self
    }

    /// Set the output sink (mutable version).
    pub fn set_output_sink(&mut self, sink: Arc<dyn OutputSink>) {
        self.output_sink = Some(sink);
    }

    fn initialize_global_variables(&mut self) -> Result<(), Xslt3Error> {
        for (name, var) in &self.stylesheet.global_variables {
            let value = self
                .evaluate_xpath31_xdm(&var.select, self.root_node, 1, 1)
                .map_err(|e| Xslt3Error::runtime(e.to_string()))?;
            self.set_variable(name.clone(), value);
        }
        Ok(())
    }

    pub fn build_tree(&mut self) -> Result<Vec<IRNode>, ExecutionError> {
        let mut builder = IdfBuilder::new();
        self.execute_with_mode(None, &mut builder)?;
        Ok(builder.get_result())
    }

    pub fn execute_with_mode(
        &mut self,
        mode: Option<&str>,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        self.apply_templates_to_nodes(&[self.root_node], mode, builder)?;
        Ok(())
    }

    pub(crate) fn push_scope(&mut self) {
        self.variable_stack.push(HashMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.variable_stack.pop();
    }

    pub(crate) fn set_variable(&mut self, name: String, value: XdmValue<N>) {
        if let Some(scope) = self.variable_stack.last_mut() {
            scope.insert(name, value);
        }
    }

    pub(crate) fn get_merged_variables(&self) -> HashMap<String, XdmValue<N>> {
        let mut merged = HashMap::new();
        for scope in &self.variable_stack {
            merged.extend(scope.clone());
        }
        merged
    }

    fn xdm_to_xpath_value(&self, xdm: &XdmValue<N>) -> XPathValue<N> {
        let items = xdm.items();
        if items.is_empty() {
            return XPathValue::String(String::new());
        }
        if items.len() == 1 {
            match &items[0] {
                XdmItem::Node(n) => XPathValue::NodeSet(vec![*n]),
                XdmItem::Atomic(a) => {
                    if let Some(b) = match a {
                        petty_xpath31::types::AtomicValue::Boolean(b) => Some(*b),
                        _ => None,
                    } {
                        XPathValue::Boolean(b)
                    } else if let Some(n) = a.to_integer() {
                        XPathValue::Number(n as f64)
                    } else {
                        XPathValue::String(a.to_string_value())
                    }
                }
                XdmItem::Map(_) | XdmItem::Array(_) | XdmItem::Function(_) => {
                    XPathValue::String(xdm.to_string_value())
                }
            }
        } else {
            let nodes: Vec<N> = items
                .iter()
                .filter_map(|item| {
                    if let XdmItem::Node(n) = item {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .collect();
            if !nodes.is_empty() {
                XPathValue::NodeSet(nodes)
            } else {
                XPathValue::String(xdm.to_string_value())
            }
        }
    }

    fn get_xpath1_variables(&self) -> HashMap<String, XPathValue<N>> {
        let merged = self.get_merged_variables();
        merged
            .iter()
            .map(|(k, v)| (k.clone(), self.xdm_to_xpath_value(v)))
            .collect()
    }

    pub(crate) fn execute_template(
        &mut self,
        template: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        for instruction in &template.0 {
            self.execute_instruction(
                instruction,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
        }
        Ok(())
    }

    fn execute_instruction(
        &mut self,
        instruction: &Xslt3Instruction,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        match instruction {
            Xslt3Instruction::Text(text) => {
                self.handle_text(text, builder);
            }
            Xslt3Instruction::TextValueTemplate(tvt) => {
                self.handle_text_value_template(
                    tvt,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ValueOf { select, separator } => {
                self.handle_value_of(
                    select,
                    separator,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Sequence { select } => {
                self.handle_sequence(select, context_node, context_position, context_size)?;
            }
            Xslt3Instruction::Variable {
                name, select, body, ..
            } => {
                self.handle_variable(
                    name,
                    select,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::If { test, body } => {
                self.handle_if(
                    test,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Choose { whens, otherwise } => {
                self.handle_choose(
                    whens,
                    otherwise,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ForEach {
                select,
                sort_keys,
                body,
            } => {
                self.handle_for_each(
                    select,
                    sort_keys,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ForEachGroup {
                select,
                group_by,
                group_adjacent,
                group_starting_with,
                group_ending_with,
                sort_keys: _,
                body,
            } => {
                self.handle_for_each_group(
                    select,
                    group_by.as_ref(),
                    group_adjacent.as_ref(),
                    group_starting_with.as_deref(),
                    group_ending_with.as_deref(),
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ApplyTemplates {
                select,
                mode,
                sort_keys,
            } => {
                self.handle_apply_templates(
                    select,
                    mode,
                    sort_keys,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::CallTemplate { name, params } => {
                self.handle_call_template(
                    name,
                    params,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ContentTag {
                tag_name,
                styles,
                attrs,
                shadow_attrs,
                use_attribute_sets,
                body,
            } => {
                self.handle_content_tag(
                    tag_name,
                    styles,
                    attrs,
                    shadow_attrs,
                    use_attribute_sets,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::EmptyTag {
                tag_name,
                styles,
                attrs,
                shadow_attrs,
                use_attribute_sets,
            } => {
                self.handle_empty_tag(
                    tag_name,
                    styles,
                    attrs,
                    shadow_attrs,
                    use_attribute_sets,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Copy { styles, body } => {
                self.handle_copy(
                    styles,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::CopyOf { select } => {
                self.handle_copy_of(
                    select,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Try {
                body,
                catches,
                rollback_output,
            } => {
                self.handle_try(
                    body,
                    catches,
                    *rollback_output,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Iterate {
                select,
                params,
                body,
                on_completion,
            } => {
                self.handle_iterate(
                    select,
                    params,
                    body,
                    on_completion,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::NextIteration { params } => {
                return self.handle_next_iteration(
                    params,
                    context_node,
                    context_position,
                    context_size,
                );
            }
            Xslt3Instruction::Break { .. } => {
                return self.handle_break();
            }
            Xslt3Instruction::Map { entries } => {
                let _ = self.handle_map(
                    entries,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::MapEntry { .. } => {}
            Xslt3Instruction::Array { members } => {
                let _ = self.handle_array(
                    members,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ArrayMember { .. } => {}
            Xslt3Instruction::Assert { test, message } => {
                self.handle_assert(test, message, context_node, context_position, context_size)?;
            }
            Xslt3Instruction::Message {
                select,
                body,
                terminate,
                error_code,
            } => {
                self.handle_message(
                    select,
                    body,
                    *terminate,
                    error_code,
                    context_node,
                    context_position,
                    context_size,
                )?;
            }
            Xslt3Instruction::OnEmpty { body } => {
                self.handle_on_empty(body, context_node, context_position, context_size, builder)?;
            }
            Xslt3Instruction::OnNonEmpty { body } => {
                self.handle_on_non_empty(
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::WherePopulated { body } => {
                self.handle_where_populated(
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::AccumulatorBefore { name } => {
                self.handle_accumulator_before(name, builder);
            }
            Xslt3Instruction::AccumulatorAfter { name } => {
                self.handle_accumulator_after(name, builder);
            }
            Xslt3Instruction::Attribute { name, body } => {
                self.handle_attribute(
                    name,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Element { name, body } => {
                self.handle_element(
                    name,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Comment { body } => {
                self.handle_comment(body, context_node, context_position, context_size, builder)?;
            }
            Xslt3Instruction::ProcessingInstruction { name: _, body } => {
                self.handle_processing_instruction(
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Namespace { .. } => {}
            Xslt3Instruction::Stream { href, body } => {
                self.handle_stream(
                    href,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::SourceDocument {
                href,
                streamable,
                body,
            } => {
                self.handle_source_document(
                    href,
                    *streamable,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Fork { branches } => {
                self.handle_fork(
                    branches,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Merge { sources, action } => {
                self.handle_merge(
                    sources,
                    action,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ResultDocument { format, href, body } => {
                self.handle_result_document(
                    format,
                    href,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::CallFunction { name, args } => {
                if let Some(func) = self.stylesheet.functions.get(name) {
                    self.push_scope();
                    for (param, arg) in func.params.iter().zip(args.iter()) {
                        let value = self.evaluate_xpath31_xdm(
                            arg,
                            context_node,
                            context_position,
                            context_size,
                        )?;
                        self.set_variable(param.name.clone(), value);
                    }
                    self.execute_template(
                        &func.body,
                        context_node,
                        context_position,
                        context_size,
                        builder,
                    )?;
                    self.pop_scope();
                } else {
                    return Err(ExecutionError::UnknownFunction(name.clone()));
                }
            }
            Xslt3Instruction::JsonToXml { select } => {
                let json_str =
                    self.evaluate_xpath31(select, context_node, context_position, context_size)?;
                if !json_str.is_empty() {
                    let parsed: serde_json::Value = serde_json::from_str(&json_str)
                        .map_err(|e| ExecutionError::XPath(format!("Invalid JSON: {}", e)))?;
                    let xml = json_to_xml_string(&parsed);
                    builder.add_text(&xml);
                }
            }
            Xslt3Instruction::XmlToJson { select } => {
                let xml_content = if let Some(expr) = select {
                    self.evaluate_xpath31(expr, context_node, context_position, context_size)?
                } else {
                    context_node.string_value()
                };
                let json = xml_to_json_string(&xml_content);
                builder.add_text(&json);
            }
            Xslt3Instruction::Evaluate {
                xpath,
                context_item,
                namespace_context: _,
            } => {
                let xpath_str =
                    self.evaluate_xpath31(xpath, context_node, context_position, context_size)?;

                let eval_context = if let Some(ctx_expr) = context_item {
                    let nodes = self.evaluate_xpath31_nodes(
                        ctx_expr,
                        context_node,
                        context_position,
                        context_size,
                    )?;
                    nodes.first().copied().unwrap_or(context_node)
                } else {
                    context_node
                };

                if let Ok(parsed) = petty_xpath31::parse_expression(&xpath_str) {
                    let result = self.evaluate_xpath31(
                        &parsed,
                        eval_context,
                        context_position,
                        context_size,
                    )?;
                    builder.add_text(&result);
                }
            }
            Xslt3Instruction::NextMatch { params } => {
                self.handle_next_match(
                    params,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::ApplyImports { params } => {
                self.handle_apply_imports(
                    params,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::AnalyzeString {
                select,
                regex,
                flags,
                matching_substring,
                non_matching_substring,
            } => {
                self.handle_analyze_string(
                    select,
                    regex,
                    flags.as_deref(),
                    matching_substring.as_ref(),
                    non_matching_substring.as_ref(),
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::PerformSort {
                select,
                sort_keys,
                body,
            } => {
                self.handle_perform_sort(
                    select.as_ref(),
                    sort_keys,
                    body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
            Xslt3Instruction::Fallback { body: _ } => {}
            Xslt3Instruction::Number {
                level,
                count,
                from,
                value,
                format,
                lang,
                letter_value,
                grouping_separator,
                grouping_size,
                ordinal,
                select,
            } => {
                self.handle_number(
                    level,
                    count,
                    from,
                    value,
                    format,
                    lang,
                    letter_value,
                    grouping_separator,
                    grouping_size,
                    ordinal,
                    select,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn evaluate_xpath31(
        &self,
        expr: &petty_xpath31::Expression,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<String, ExecutionError> {
        let result =
            self.evaluate_xpath31_xdm(expr, context_node, context_position, context_size)?;
        Ok(result.to_xpath_string())
    }

    pub(crate) fn evaluate_xpath31_xdm(
        &self,
        expr: &petty_xpath31::Expression,
        context_node: N,
        _context_position: usize,
        _context_size: usize,
    ) -> Result<XdmValue<N>, ExecutionError> {
        let mut xdm_vars = self.get_merged_variables();

        if let Some(key) = &self.current_grouping_key {
            xdm_vars.insert(
                "::current-grouping-key".to_string(),
                XdmValue::from_string(key.clone()),
            );
        }
        if !self.current_group.is_empty() {
            let items: Vec<XdmItem<N>> = self
                .current_group
                .iter()
                .map(|n| XdmItem::Node(*n))
                .collect();
            xdm_vars.insert("::current-group".to_string(), XdmValue::from_items(items));
        }

        if let Some(key) = &self.current_merge_key {
            xdm_vars.insert(
                "::current-merge-key".to_string(),
                XdmValue::from_string(key.clone()),
            );
        }
        if !self.current_merge_group.is_empty() {
            let items: Vec<XdmItem<N>> = self
                .current_merge_group
                .iter()
                .map(|n| XdmItem::Node(*n))
                .collect();
            xdm_vars.insert(
                "::current-merge-group".to_string(),
                XdmValue::from_items(items),
            );
        }
        if let Some(source) = &self.current_merge_source {
            xdm_vars.insert(
                "::current-merge-source".to_string(),
                XdmValue::from_string(source.clone()),
            );
        }

        for (key_name, key_index) in &self.key_indexes {
            let mut map_entries = Vec::new();
            for (value, nodes) in key_index {
                let key = petty_xpath31::types::AtomicValue::String(value.clone());
                let node_items: Vec<XdmItem<N>> = nodes.iter().map(|n| XdmItem::Node(*n)).collect();
                map_entries.push((key, XdmValue::from_items(node_items)));
            }
            let xdm_map = petty_xpath31::XdmMap::from_entries(map_entries);
            xdm_vars.insert(
                format!("::key-index:{}", key_name),
                XdmValue::from_items(vec![XdmItem::Map(xdm_map)]),
            );
        }

        for (name, df) in &self.stylesheet.decimal_formats {
            let var_name = match name {
                Some(n) => format!("::decimal-format:{}", n),
                None => "::decimal-format:".to_string(),
            };
            let encoded = encode_decimal_format(df);
            xdm_vars.insert(var_name, XdmValue::from_string(encoded));
        }

        let context_item = if let Some(ref match_str) = self.regex_match {
            Some(XdmItem::Atomic(petty_xpath31::types::AtomicValue::String(
                match_str.clone(),
            )))
        } else {
            Some(XdmItem::Node(context_node))
        };
        let root = Some(self.root_node);

        let xdm_ctx = petty_xpath31::EvaluationContext::new(context_item, root, &xdm_vars);

        let result = petty_xpath31::evaluate(expr, &xdm_ctx, &xdm_vars)?;
        Ok(result)
    }

    pub(crate) fn evaluate_xpath31_nodes(
        &self,
        expr: &petty_xpath31::Expression,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<Vec<N>, ExecutionError> {
        let result =
            self.evaluate_xpath31_xdm(expr, context_node, context_position, context_size)?;
        Ok(result.to_nodes())
    }

    pub(crate) fn evaluate_tvt(
        &self,
        tvt: &TextValueTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<String, ExecutionError> {
        let mut result = String::new();
        for part in &tvt.0 {
            match part {
                TvtPart::Static(s) => result.push_str(s),
                TvtPart::Dynamic(expr) => {
                    let value =
                        self.evaluate_xpath31(expr, context_node, context_position, context_size)?;
                    result.push_str(&value);
                }
            }
        }
        Ok(result)
    }

    pub(crate) fn evaluate_avt(
        &self,
        avt: &petty_xslt::ast::AttributeValueTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<String, ExecutionError> {
        match avt {
            petty_xslt::ast::AttributeValueTemplate::Static(s) => Ok(s.clone()),
            petty_xslt::ast::AttributeValueTemplate::Dynamic(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        petty_xslt::ast::AvtPart::Static(s) => result.push_str(s),
                        petty_xslt::ast::AvtPart::Dynamic(expr) => {
                            let xpath1_vars = self.get_xpath1_variables();
                            let empty_key_map = HashMap::new();
                            let xpath1_ctx = petty_xpath1::engine::EvaluationContext::new(
                                context_node,
                                self.root_node,
                                &self.functions,
                                context_position,
                                context_size,
                                &xpath1_vars,
                                &empty_key_map,
                                self.strict,
                            );
                            let value = petty_xpath1::evaluate(expr, &xpath1_ctx)?;
                            result.push_str(&value.to_string());
                        }
                    }
                }
                Ok(result)
            }
        }
    }

    pub(crate) fn evaluate_avt3(
        &self,
        avt: &crate::ast::Avt3,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<String, ExecutionError> {
        match avt {
            crate::ast::Avt3::Static(s) => Ok(s.clone()),
            crate::ast::Avt3::Dynamic(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        crate::ast::Avt3Part::Static(s) => result.push_str(s),
                        crate::ast::Avt3Part::Dynamic(expr) => {
                            let value = self.evaluate_xpath31(
                                expr,
                                context_node,
                                context_position,
                                context_size,
                            )?;
                            result.push_str(&value);
                        }
                    }
                }
                Ok(result)
            }
        }
    }

    pub(crate) fn apply_templates_to_nodes(
        &mut self,
        nodes: &[N],
        mode: Option<&str>,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let filtered_nodes: Vec<N> = nodes
            .iter()
            .copied()
            .filter(|&node| !self.should_strip_whitespace_node(node))
            .collect();

        let context_size = filtered_nodes.len();
        let prev_mode = self.current_mode.clone();
        self.current_mode = mode.map(String::from);

        for (i, &node) in filtered_nodes.iter().enumerate() {
            let context_position = i + 1;

            self.process_accumulator_node(
                node,
                context_position,
                context_size,
                AccumulatorPhase::Start,
            )?;

            if let Some((idx, rule)) = self.find_matching_template_with_index(node, mode) {
                let body = rule.body.clone();
                let prev_idx = self.current_template_index;
                self.current_template_index = Some(idx);

                self.push_scope();
                self.execute_template(&body, node, context_position, context_size, builder)?;
                self.pop_scope();

                self.current_template_index = prev_idx;
            } else {
                self.apply_builtin_template(node, builder)?;
            }

            self.process_accumulator_node(
                node,
                context_position,
                context_size,
                AccumulatorPhase::End,
            )?;
        }

        self.current_mode = prev_mode;
        Ok(())
    }

    pub(crate) fn apply_builtin_template(
        &mut self,
        node: N,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let on_no_match = self.get_current_mode_on_no_match();
        let current_mode = self.current_mode.clone();

        match on_no_match {
            OnNoMatch::DeepSkip => Ok(()),
            OnNoMatch::ShallowSkip => {
                match node.node_type() {
                    NodeType::Root | NodeType::Element => {
                        let children: Vec<N> = node.children().collect();
                        self.apply_templates_to_nodes(&children, current_mode.as_deref(), builder)?;
                    }
                    _ => {}
                }
                Ok(())
            }
            OnNoMatch::TextOnlyCopy => {
                match node.node_type() {
                    NodeType::Root | NodeType::Element => {
                        let children: Vec<N> = node.children().collect();
                        self.apply_templates_to_nodes(&children, current_mode.as_deref(), builder)?;
                    }
                    NodeType::Text | NodeType::Attribute => {
                        builder.add_text(&node.string_value());
                    }
                    NodeType::Comment | NodeType::ProcessingInstruction => {}
                }
                Ok(())
            }
            OnNoMatch::ShallowCopy => {
                match node.node_type() {
                    NodeType::Element => {
                        builder.start_block(&PreparsedStyles::default());
                        let children: Vec<N> = node.children().collect();
                        self.apply_templates_to_nodes(&children, current_mode.as_deref(), builder)?;
                        builder.end_block();
                    }
                    NodeType::Root => {
                        let children: Vec<N> = node.children().collect();
                        self.apply_templates_to_nodes(&children, current_mode.as_deref(), builder)?;
                    }
                    NodeType::Text => {
                        builder.add_text(&node.string_value());
                    }
                    NodeType::Attribute | NodeType::Comment | NodeType::ProcessingInstruction => {}
                }
                Ok(())
            }
            OnNoMatch::DeepCopy => self.deep_copy_node(node, builder),
            OnNoMatch::Fail => {
                let node_name = node
                    .name()
                    .map(|q| q.local_part.to_string())
                    .unwrap_or_else(|| format!("{:?}", node.node_type()));
                Err(ExecutionError::NoMatchingTemplate { node_name })
            }
        }
    }

    fn get_current_mode_on_no_match(&self) -> OnNoMatch {
        self.stylesheet
            .modes
            .get(&self.current_mode)
            .map(|m| m.on_no_match)
            .unwrap_or(OnNoMatch::TextOnlyCopy)
    }

    fn deep_copy_node(
        &mut self,
        node: N,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        match node.node_type() {
            NodeType::Element => {
                builder.start_block(&PreparsedStyles::default());
                for child in node.children() {
                    self.deep_copy_node(child, builder)?;
                }
                builder.end_block();
            }
            NodeType::Text => {
                builder.add_text(&node.string_value());
            }
            NodeType::Root => {
                for child in node.children() {
                    self.deep_copy_node(child, builder)?;
                }
            }
            NodeType::Attribute | NodeType::Comment | NodeType::ProcessingInstruction => {}
        }
        Ok(())
    }

    fn should_strip_whitespace_node(&self, node: N) -> bool {
        if node.node_type() != NodeType::Text {
            return false;
        }

        let text = node.string_value();
        if !text.chars().all(char::is_whitespace) {
            return false;
        }

        let parent_name = node
            .parent()
            .and_then(|p| p.name())
            .map(|q| q.local_part)
            .unwrap_or("");

        let in_preserve =
            self.element_matches_whitespace_pattern(parent_name, &self.stylesheet.preserve_space);
        let in_strip =
            self.element_matches_whitespace_pattern(parent_name, &self.stylesheet.strip_space);

        in_strip && !in_preserve
    }

    fn element_matches_whitespace_pattern(&self, element_name: &str, patterns: &[String]) -> bool {
        for pattern in patterns {
            if pattern == "*" || pattern == element_name {
                return true;
            }
        }
        false
    }

    #[allow(dead_code)]
    fn find_matching_template(&self, node: N, mode: Option<&str>) -> Option<&'s TemplateRule3> {
        self.find_matching_template_with_index(node, mode)
            .map(|(_, rule)| rule)
    }

    fn find_matching_template_with_index(
        &self,
        node: N,
        mode: Option<&str>,
    ) -> Option<(usize, &'s TemplateRule3)> {
        let rules_for_mode = self
            .stylesheet
            .template_rules
            .get(&mode.map(String::from))?;

        rules_for_mode
            .iter()
            .enumerate()
            .find(|(_, rule)| self.pattern_matches(&rule.pattern.0, node))
    }

    pub(crate) fn pattern_matches(&self, pattern: &str, node: N) -> bool {
        match node.node_type() {
            NodeType::Root => pattern == "/" || pattern == "/*",
            NodeType::Element => {
                if pattern == "*" || pattern == "node()" {
                    return true;
                }
                if let Some(qname) = node.name() {
                    let name = qname.local_part;
                    pattern == name || pattern == "*" || pattern.ends_with(&format!("/{}", name))
                } else {
                    false
                }
            }
            NodeType::Text => pattern == "text()" || pattern == "node()",
            NodeType::Attribute => {
                if let Some(attr_pattern) = pattern.strip_prefix('@') {
                    if let Some(qname) = node.name() {
                        attr_pattern == "*" || attr_pattern == qname.local_part
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            NodeType::Comment => pattern == "comment()" || pattern == "node()",
            NodeType::ProcessingInstruction => {
                pattern == "processing-instruction()" || pattern == "node()"
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_stream(
        &mut self,
        href: &AttributeValueTemplate,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let href_value = self.evaluate_avt(href, context_node, context_position, context_size)?;

        let provider = self.resource_provider.as_ref().ok_or_else(|| {
            ExecutionError::Stream("No resource provider configured for xsl:stream".into())
        })?;

        let xml_content = provider.load(&href_value).map_err(|e| {
            ExecutionError::Resource(format!("Failed to load '{}': {}", href_value, e))
        })?;

        let xml_str = String::from_utf8_lossy(&xml_content);

        let has_streaming_features = !self.stylesheet.accumulators.is_empty()
            || self.stylesheet.modes.values().any(|m| m.streamable);

        if has_streaming_features {
            let streaming_result = parse_and_stream_with_accumulators(&xml_str, self.stylesheet)
                .map_err(|e| ExecutionError::Stream(e.to_string()))?;

            self.accumulator_values = streaming_result.accumulator_values;

            if body.0.is_empty() {
                self.write_ir_nodes_to_builder(&streaming_result.ir_nodes, builder);
            } else {
                self.execute_template(body, context_node, context_position, context_size, builder)?;
            }
            Ok(())
        } else {
            self.execute_template(body, context_node, context_position, context_size, builder)
        }
    }

    fn write_ir_nodes_to_builder(&self, nodes: &[IRNode], builder: &mut dyn OutputBuilder) {
        for node in nodes {
            match node {
                IRNode::Block { meta, children } => {
                    let styles = PreparsedStyles::from_meta(meta);
                    builder.start_block(&styles);
                    self.write_ir_nodes_to_builder(children, builder);
                    builder.end_block();
                }
                IRNode::Paragraph { meta, children } => {
                    let styles = PreparsedStyles::from_meta(meta);
                    builder.start_paragraph(&styles);
                    self.write_inline_nodes_to_builder(children, builder);
                    builder.end_paragraph();
                }
                IRNode::Heading {
                    meta,
                    level,
                    children,
                } => {
                    let styles = PreparsedStyles::from_meta(meta);
                    builder.start_heading(&styles, *level);
                    self.write_inline_nodes_to_builder(children, builder);
                    builder.end_heading();
                }
                IRNode::FlexContainer { meta, children } => {
                    let styles = PreparsedStyles::from_meta(meta);
                    builder.start_flex_container(&styles);
                    self.write_ir_nodes_to_builder(children, builder);
                    builder.end_flex_container();
                }
                IRNode::List { meta, children, .. } => {
                    let styles = PreparsedStyles::from_meta(meta);
                    builder.start_list(&styles);
                    self.write_ir_nodes_to_builder(children, builder);
                    builder.end_list();
                }
                IRNode::ListItem { meta, children } => {
                    let styles = PreparsedStyles::from_meta(meta);
                    builder.start_list_item(&styles);
                    self.write_ir_nodes_to_builder(children, builder);
                    builder.end_list_item();
                }
                IRNode::PageBreak { master_name } => {
                    builder.add_page_break(master_name.clone());
                }
                _ => {}
            }
        }
    }

    fn write_inline_nodes_to_builder(
        &self,
        nodes: &[petty_idf::InlineNode],
        builder: &mut dyn OutputBuilder,
    ) {
        use petty_xslt::ast::PreparsedStyles;

        for node in nodes {
            match node {
                petty_idf::InlineNode::Text(text) => {
                    builder.add_text(text);
                }
                petty_idf::InlineNode::StyledSpan { meta, children } => {
                    let styles = PreparsedStyles::from_inline_meta(meta);
                    builder.start_styled_span(&styles);
                    self.write_inline_nodes_to_builder(children, builder);
                    builder.end_styled_span();
                }
                petty_idf::InlineNode::Hyperlink {
                    href,
                    meta,
                    children,
                } => {
                    let styles = PreparsedStyles::from_inline_meta(meta);
                    builder.start_hyperlink(&styles);
                    builder.set_attribute("href", href);
                    self.write_inline_nodes_to_builder(children, builder);
                    builder.end_hyperlink();
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_source_document(
        &mut self,
        href: &AttributeValueTemplate,
        streamable: bool,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let href_value = self.evaluate_avt(href, context_node, context_position, context_size)?;

        let provider = self.resource_provider.as_ref().ok_or_else(|| {
            ExecutionError::Stream("No resource provider configured for xsl:source-document".into())
        })?;

        let xml_content = provider.load(&href_value).map_err(|e| {
            ExecutionError::Resource(format!("Failed to load '{}': {}", href_value, e))
        })?;

        let xml_str = String::from_utf8_lossy(&xml_content);

        if streamable {
            let has_streaming_features = !self.stylesheet.accumulators.is_empty()
                || self.stylesheet.modes.values().any(|m| m.streamable);

            if has_streaming_features {
                let streaming_result =
                    parse_and_stream_with_accumulators(&xml_str, self.stylesheet)
                        .map_err(|e| ExecutionError::Stream(e.to_string()))?;

                self.accumulator_values = streaming_result.accumulator_values;

                if body.0.is_empty() {
                    self.write_ir_nodes_to_builder(&streaming_result.ir_nodes, builder);
                } else {
                    self.execute_template(
                        body,
                        context_node,
                        context_position,
                        context_size,
                        builder,
                    )?;
                }
                return Ok(());
            }
        }

        self.execute_template(body, context_node, context_position, context_size, builder)
    }

    pub fn process_xml_streaming(&self, xml_content: &str) -> Result<Vec<IRNode>, ExecutionError> {
        parse_and_stream(xml_content, self.stylesheet)
            .map_err(|e| ExecutionError::XPath(e.to_string()))
    }

    pub(crate) fn execute_start_tag(
        &self,
        tag_name: &[u8],
        styles: &PreparsedStyles,
        builder: &mut dyn OutputBuilder,
    ) {
        match String::from_utf8_lossy(tag_name).as_ref() {
            "p" => builder.start_paragraph(styles),
            "fo:block" | "block" | "root" | "div" => builder.start_block(styles),
            "fo:flex-container" | "flex-container" => builder.start_flex_container(styles),
            "fo:list-block" | "list" => builder.start_list(styles),
            "fo:list-item" | "list-item" => builder.start_list_item(styles),
            "fo:inline" | "span" | "strong" | "b" | "em" | "i" => builder.start_styled_span(styles),
            "fo:basic-link" | "fo:link" | "a" | "link" => builder.start_hyperlink(styles),
            "fo:external-graphic" | "img" => builder.start_image(styles),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                builder.start_heading(styles, tag_name.last().map_or(1, |&c| c - b'0'))
            }
            "table" | "fo:table" => builder.start_table(styles),
            "tbody" | "thead" | "header" => {}
            "tr" | "row" | "fo:table-row" => builder.start_table_row(styles),
            "td" | "cell" | "fo:table-cell" => builder.start_table_cell(styles),
            _ => builder.start_block(styles),
        };
    }

    pub(crate) fn execute_end_tag(&self, tag_name: &[u8], builder: &mut dyn OutputBuilder) {
        match String::from_utf8_lossy(tag_name).as_ref() {
            "p" => builder.end_paragraph(),
            "fo:block" | "block" | "root" | "div" => builder.end_block(),
            "fo:flex-container" | "flex-container" => builder.end_flex_container(),
            "fo:list-block" | "list" => builder.end_list(),
            "fo:list-item" | "list-item" => builder.end_list_item(),
            "fo:inline" | "span" | "strong" | "b" | "em" | "i" => builder.end_styled_span(),
            "fo:basic-link" | "fo:link" | "a" | "link" => builder.end_hyperlink(),
            "fo:external-graphic" | "img" => builder.end_image(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => builder.end_heading(),
            "table" | "fo:table" => builder.end_table(),
            "tbody" | "thead" | "header" => {}
            "tr" | "row" | "fo:table-row" => builder.end_table_row(),
            "td" | "cell" | "fo:table-cell" => builder.end_table_cell(),
            _ => builder.end_block(),
        }
    }

    pub(crate) fn add_text_with_character_maps(&self, text: &str, builder: &mut dyn OutputBuilder) {
        let use_maps = &self.stylesheet.output.use_character_maps;
        if use_maps.is_empty() {
            builder.add_text(text);
        } else {
            let mapped = apply_character_maps(text, &self.stylesheet.character_maps, use_maps);
            builder.add_text(&mapped);
        }
    }
}

fn json_to_xml_string(value: &serde_json::Value) -> String {
    let mut result = String::new();
    result.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    result.push_str(r#"<map xmlns="http://www.w3.org/2005/xpath-functions">"#);
    json_to_xml_inner(value, &mut result);
    result.push_str("</map>");
    result
}

fn json_to_xml_inner(value: &serde_json::Value, result: &mut String) {
    match value {
        serde_json::Value::Null => result.push_str("<null/>"),
        serde_json::Value::Bool(b) => {
            result.push_str(&format!("<boolean>{}</boolean>", b));
        }
        serde_json::Value::Number(n) => {
            result.push_str(&format!("<number>{}</number>", n));
        }
        serde_json::Value::String(s) => {
            result.push_str(&format!("<string>{}</string>", escape_xml(s)));
        }
        serde_json::Value::Array(arr) => {
            result.push_str("<array>");
            for item in arr {
                json_to_xml_inner(item, result);
            }
            result.push_str("</array>");
        }
        serde_json::Value::Object(obj) => {
            result.push_str("<map>");
            for (key, val) in obj {
                let escaped_key = escape_xml(key);
                result.push_str(&format!(r#"<entry key="{}">"#, escaped_key));
                json_to_xml_inner(val, result);
                result.push_str("</entry>");
            }
            result.push_str("</map>");
        }
    }
}

fn xml_to_json_string(xml: &str) -> String {
    let trimmed = xml.trim();
    if trimmed.is_empty() {
        return "null".to_string();
    }

    if !trimmed.starts_with('<') {
        return format!("\"{}\"", escape_json(trimmed));
    }

    format!("\"{}\"", escape_json(trimmed))
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn encode_decimal_format(df: &crate::ast::DecimalFormatDeclaration) -> String {
    format!(
        "ds={}\x1Fgs={}\x1Fms={}\x1Fpc={}\x1Fpm={}\x1Fzd={}\x1Fdg={}\x1Fps={}\x1Finf={}\x1Fnan={}",
        df.decimal_separator,
        df.grouping_separator,
        df.minus_sign,
        df.percent,
        df.per_mille,
        df.zero_digit,
        df.digit,
        df.pattern_separator,
        df.infinity,
        df.nan
    )
}

fn apply_character_maps(
    text: &str,
    character_maps: &HashMap<String, crate::ast::CharacterMap>,
    use_maps: &[String],
) -> String {
    if use_maps.is_empty() || character_maps.is_empty() {
        return text.to_string();
    }

    let mut merged_mappings: HashMap<char, &str> = HashMap::new();

    fn collect_mappings<'a>(
        map_name: &str,
        character_maps: &'a HashMap<String, crate::ast::CharacterMap>,
        merged: &mut HashMap<char, &'a str>,
        visited: &mut Vec<String>,
    ) {
        if visited.contains(&map_name.to_string()) {
            return;
        }
        visited.push(map_name.to_string());

        if let Some(char_map) = character_maps.get(map_name) {
            for referenced in &char_map.use_character_maps {
                collect_mappings(referenced, character_maps, merged, visited);
            }
            for mapping in &char_map.mappings {
                merged.insert(mapping.character, &mapping.string);
            }
        }
    }

    let mut visited = Vec::new();
    for map_name in use_maps {
        collect_mappings(map_name, character_maps, &mut merged_mappings, &mut visited);
    }

    if merged_mappings.is_empty() {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        if let Some(replacement) = merged_mappings.get(&c) {
            result.push_str(replacement);
        } else {
            result.push(c);
        }
    }
    result
}
