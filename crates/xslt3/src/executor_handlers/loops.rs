#![allow(clippy::too_many_arguments)]

use crate::ast::{IterateParam, NextIterationParam, PreparsedTemplate, SortKey3, WithParam3};
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::types::XdmValue;
use petty_xslt::ast::{AttributeValueTemplate, SortDataType, SortOrder};
use petty_xslt::output::OutputBuilder;
use std::cmp::Ordering;
use std::collections::HashMap;

enum SortValue {
    Text(String),
    Number(f64),
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub(crate) fn handle_for_each_group(
        &mut self,
        select: &petty_xpath31::Expression,
        group_by: Option<&petty_xpath31::Expression>,
        group_adjacent: Option<&petty_xpath31::Expression>,
        group_starting_with: Option<&str>,
        group_ending_with: Option<&str>,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let nodes =
            self.evaluate_xpath31_nodes(select, context_node, context_position, context_size)?;

        let groups_with_keys: Vec<(String, Vec<N>)> =
            if let Some(group_adjacent_expr) = group_adjacent {
                self.group_adjacent(&nodes, group_adjacent_expr)?
            } else if let Some(pattern) = group_starting_with {
                self.group_starting_with(&nodes, pattern)?
            } else if let Some(pattern) = group_ending_with {
                self.group_ending_with(&nodes, pattern)?
            } else if let Some(group_by_expr) = group_by {
                self.group_by(&nodes, group_by_expr)?
            } else {
                self.group_by_string_value(&nodes)
            };

        let group_count = groups_with_keys.len();
        for (i, (key, group_nodes)) in groups_with_keys.into_iter().enumerate() {
            self.current_grouping_key = Some(key);
            self.current_group = group_nodes.clone();

            let first_node = group_nodes.first().copied().unwrap_or(context_node);
            self.push_scope();
            self.execute_template(body, first_node, i + 1, group_count, builder)?;
            self.pop_scope();
        }

        self.current_grouping_key = None;
        self.current_group.clear();

        Ok(())
    }

    fn group_by(
        &mut self,
        nodes: &[N],
        group_by_expr: &petty_xpath31::Expression,
    ) -> Result<Vec<(String, Vec<N>)>, ExecutionError> {
        let mut groups: HashMap<String, Vec<N>> = HashMap::new();
        let mut group_order: Vec<String> = Vec::new();

        for node in nodes {
            let key = self.evaluate_xpath31(group_by_expr, *node, 1, 1)?;
            if !groups.contains_key(&key) {
                group_order.push(key.clone());
            }
            groups.entry(key).or_default().push(*node);
        }

        Ok(group_order
            .into_iter()
            .map(|key| {
                let nodes = groups.remove(&key).unwrap_or_default();
                (key, nodes)
            })
            .collect())
    }

    fn group_by_string_value(&self, nodes: &[N]) -> Vec<(String, Vec<N>)> {
        let mut groups: HashMap<String, Vec<N>> = HashMap::new();
        let mut group_order: Vec<String> = Vec::new();

        for node in nodes {
            let key = node.string_value();
            if !groups.contains_key(&key) {
                group_order.push(key.clone());
            }
            groups.entry(key).or_default().push(*node);
        }

        group_order
            .into_iter()
            .map(|key| {
                let nodes = groups.remove(&key).unwrap_or_default();
                (key, nodes)
            })
            .collect()
    }

    fn group_adjacent(
        &mut self,
        nodes: &[N],
        group_adjacent_expr: &petty_xpath31::Expression,
    ) -> Result<Vec<(String, Vec<N>)>, ExecutionError> {
        let mut result: Vec<(String, Vec<N>)> = Vec::new();

        for node in nodes {
            let key = self.evaluate_xpath31(group_adjacent_expr, *node, 1, 1)?;

            match result.last_mut() {
                Some(last_group) if last_group.0 == key => last_group.1.push(*node),
                _ => result.push((key, vec![*node])),
            }
        }

        Ok(result)
    }

    fn group_starting_with(
        &mut self,
        nodes: &[N],
        pattern: &str,
    ) -> Result<Vec<(String, Vec<N>)>, ExecutionError> {
        let mut result: Vec<(String, Vec<N>)> = Vec::new();
        let mut group_counter = 0;

        for node in nodes {
            let matches_pattern = self.node_matches_pattern(*node, pattern)?;

            if matches_pattern || result.is_empty() {
                group_counter += 1;
                result.push((format!("group-{}", group_counter), vec![*node]));
            } else if let Some(last_group) = result.last_mut() {
                last_group.1.push(*node);
            }
        }

        Ok(result)
    }

    fn group_ending_with(
        &mut self,
        nodes: &[N],
        pattern: &str,
    ) -> Result<Vec<(String, Vec<N>)>, ExecutionError> {
        let mut result: Vec<(String, Vec<N>)> = Vec::new();
        let mut current_group: Vec<N> = Vec::new();
        let mut group_counter = 0;

        for node in nodes {
            current_group.push(*node);

            if self.node_matches_pattern(*node, pattern)? {
                group_counter += 1;
                result.push((
                    format!("group-{}", group_counter),
                    std::mem::take(&mut current_group),
                ));
            }
        }

        if !current_group.is_empty() {
            group_counter += 1;
            result.push((format!("group-{}", group_counter), current_group));
        }

        Ok(result)
    }

    fn node_matches_pattern(&self, node: N, pattern: &str) -> Result<bool, ExecutionError> {
        let pattern = pattern.trim();

        if pattern == "*" {
            return Ok(true);
        }

        if pattern.contains('|') {
            for sub_pattern in pattern.split('|') {
                if self.node_matches_pattern(node, sub_pattern.trim())? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }

        let node_name = node.name().map(|q| q.local_part).unwrap_or("");
        Ok(node_name == pattern)
    }

    fn apply_sort_keys(
        &mut self,
        mut nodes: Vec<N>,
        sort_keys: &[SortKey3],
    ) -> Result<Vec<N>, ExecutionError> {
        if sort_keys.is_empty() {
            return Ok(nodes);
        }

        let mut sort_data: Vec<(N, Vec<SortValue>)> = Vec::with_capacity(nodes.len());

        for node in nodes.drain(..) {
            let mut values = Vec::with_capacity(sort_keys.len());
            for key in sort_keys {
                let val = self.evaluate_xpath31(&key.select, node, 1, 1)?;
                let sort_val = match key.data_type {
                    SortDataType::Number => {
                        SortValue::Number(val.parse::<f64>().unwrap_or(f64::NAN))
                    }
                    SortDataType::Text => SortValue::Text(val),
                };
                values.push(sort_val);
            }
            sort_data.push((node, values));
        }

        sort_data.sort_by(|a, b| {
            for (i, (val_a, val_b)) in a.1.iter().zip(b.1.iter()).enumerate() {
                let cmp = match (val_a, val_b) {
                    (SortValue::Text(ta), SortValue::Text(tb)) => ta.cmp(tb),
                    (SortValue::Number(na), SortValue::Number(nb)) => {
                        na.partial_cmp(nb).unwrap_or(Ordering::Equal)
                    }
                    _ => Ordering::Equal,
                };
                if cmp != Ordering::Equal {
                    return if sort_keys[i].order == SortOrder::Descending {
                        cmp.reverse()
                    } else {
                        cmp
                    };
                }
            }
            Ordering::Equal
        });

        Ok(sort_data.into_iter().map(|(node, _)| node).collect())
    }

    pub(crate) fn handle_for_each(
        &mut self,
        select: &petty_xpath31::Expression,
        sort_keys: &[SortKey3],
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let nodes =
            self.evaluate_xpath31_nodes(select, context_node, context_position, context_size)?;

        let sorted_nodes = self.apply_sort_keys(nodes, sort_keys)?;
        let new_size = sorted_nodes.len();

        for (i, node) in sorted_nodes.iter().enumerate() {
            self.push_scope();
            self.execute_template(body, *node, i + 1, new_size, builder)?;
            self.pop_scope();
        }

        Ok(())
    }

    pub(crate) fn handle_apply_templates(
        &mut self,
        select: &Option<petty_xpath31::Expression>,
        mode: &Option<AttributeValueTemplate>,
        sort_keys: &[SortKey3],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let mode_str = if let Some(avt) = mode {
            Some(self.evaluate_avt(avt, context_node, context_position, context_size)?)
        } else {
            None
        };

        let nodes: Vec<N> = if let Some(select_expr) = select {
            self.evaluate_xpath31_nodes(select_expr, context_node, context_position, context_size)?
        } else {
            context_node.children().collect()
        };

        let sorted_nodes = self.apply_sort_keys(nodes, sort_keys)?;
        self.apply_templates_to_nodes(&sorted_nodes, mode_str.as_deref(), builder)?;

        Ok(())
    }

    pub(crate) fn handle_call_template(
        &mut self,
        name: &str,
        params: &[WithParam3],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let template = self
            .stylesheet
            .named_templates
            .get(name)
            .ok_or_else(|| ExecutionError::UnknownNamedTemplate(name.to_string()))?
            .clone();

        self.push_scope();

        for param in &template.params {
            let value = if let Some(with_param) = params.iter().find(|p| p.name == param.name) {
                self.evaluate_xpath31_xdm(
                    &with_param.select,
                    context_node,
                    context_position,
                    context_size,
                )?
            } else if let Some(default) = &param.default_value {
                self.evaluate_xpath31_xdm(default, context_node, context_position, context_size)?
            } else {
                XdmValue::from_string(String::new())
            };
            self.set_variable(param.name.clone(), value);
        }

        self.execute_template(
            &template.body,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
        self.pop_scope();

        Ok(())
    }

    pub(crate) fn handle_iterate(
        &mut self,
        select: &petty_xpath31::Expression,
        params: &[IterateParam],
        body: &PreparsedTemplate,
        on_completion: &Option<PreparsedTemplate>,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let nodes =
            self.evaluate_xpath31_nodes(select, context_node, context_position, context_size)?;
        let new_size = nodes.len();

        self.push_scope();

        for param in params {
            let initial = self.evaluate_xpath31_xdm(
                &param.select,
                context_node,
                context_position,
                context_size,
            )?;
            self.set_variable(param.name.clone(), initial);
        }

        for (i, node) in nodes.iter().enumerate() {
            match self.execute_template(body, *node, i + 1, new_size, builder) {
                Ok(()) => {}
                Err(ExecutionError::Break) => {
                    self.pop_scope();
                    return Ok(());
                }
                Err(ExecutionError::NextIteration(new_params)) => {
                    for (name, value) in new_params {
                        self.set_variable(name, XdmValue::from_string(value));
                    }
                }
                Err(e) => {
                    self.pop_scope();
                    return Err(e);
                }
            }
        }

        if let Some(completion) = on_completion {
            self.execute_template(
                completion,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
        }

        self.pop_scope();
        Ok(())
    }

    pub(crate) fn handle_next_iteration(
        &mut self,
        params: &[NextIterationParam],
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<(), ExecutionError> {
        let mut param_values = Vec::new();
        for param in params {
            let value =
                self.evaluate_xpath31(&param.select, context_node, context_position, context_size)?;
            param_values.push((param.name.clone(), value));
        }
        Err(ExecutionError::NextIteration(param_values))
    }

    pub(crate) fn handle_break(&self) -> Result<(), ExecutionError> {
        Err(ExecutionError::Break)
    }
}
