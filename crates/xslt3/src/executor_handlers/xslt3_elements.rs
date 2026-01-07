use crate::ast::{
    ArrayMemberInstruction, MapEntryInstruction, PreparsedTemplate, SortKey3, WithParam3,
};
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_style::dimension::Dimension;
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::types::{AtomicValue, XdmArray, XdmItem, XdmMap, XdmValue};
use petty_xslt::ast::{AttributeValueTemplate, PreparsedStyles};
use petty_xslt::buffering_builder::BufferingOutputBuilder;
use petty_xslt::output::OutputBuilder;
use regex::Regex;

struct TextCollector {
    text: String,
}

impl TextCollector {
    fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    fn into_text(self) -> String {
        self.text
    }
}

impl OutputBuilder for TextCollector {
    fn start_block(&mut self, _styles: &PreparsedStyles) {}
    fn end_block(&mut self) {}
    fn start_flex_container(&mut self, _styles: &PreparsedStyles) {}
    fn end_flex_container(&mut self) {}
    fn start_paragraph(&mut self, _styles: &PreparsedStyles) {}
    fn end_paragraph(&mut self) {}
    fn start_list(&mut self, _styles: &PreparsedStyles) {}
    fn end_list(&mut self) {}
    fn start_list_item(&mut self, _styles: &PreparsedStyles) {}
    fn end_list_item(&mut self) {}
    fn start_image(&mut self, _styles: &PreparsedStyles) {}
    fn end_image(&mut self) {}
    fn start_table(&mut self, _styles: &PreparsedStyles) {}
    fn end_table(&mut self) {}
    fn start_table_header(&mut self) {}
    fn end_table_header(&mut self) {}
    fn set_table_columns(&mut self, _columns: &[Dimension]) {}
    fn start_table_row(&mut self, _styles: &PreparsedStyles) {}
    fn end_table_row(&mut self) {}
    fn start_table_cell(&mut self, _styles: &PreparsedStyles) {}
    fn end_table_cell(&mut self) {}
    fn add_text(&mut self, text: &str) {
        self.text.push_str(text);
    }
    fn start_heading(&mut self, _styles: &PreparsedStyles, _level: u8) {}
    fn end_heading(&mut self) {}
    fn add_page_break(&mut self, _master_name: Option<String>) {}
    fn start_styled_span(&mut self, _styles: &PreparsedStyles) {}
    fn end_styled_span(&mut self) {}
    fn start_hyperlink(&mut self, _styles: &PreparsedStyles) {}
    fn end_hyperlink(&mut self) {}
    fn set_attribute(&mut self, _name: &str, _value: &str) {}
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub(crate) fn handle_map(
        &mut self,
        entries: &[MapEntryInstruction],
        context_node: N,
        context_position: usize,
        context_size: usize,
        _builder: &mut dyn OutputBuilder,
    ) -> Result<XdmValue<N>, ExecutionError> {
        let mut map_entries: Vec<(AtomicValue, XdmValue<N>)> = Vec::with_capacity(entries.len());

        for entry in entries {
            let key_xdm = self.evaluate_xpath31_xdm(
                &entry.key,
                context_node,
                context_position,
                context_size,
            )?;

            let key = self.xdm_to_atomic_key(&key_xdm)?;

            let value = if let Some(sel) = &entry.select {
                self.evaluate_xpath31_xdm(sel, context_node, context_position, context_size)?
            } else if let Some(body) = &entry.body {
                let mut body_builder = TextCollector::new();
                self.execute_template(
                    body,
                    context_node,
                    context_position,
                    context_size,
                    &mut body_builder,
                )?;
                XdmValue::from_string(body_builder.into_text())
            } else {
                XdmValue::empty()
            };

            map_entries.push((key, value));
        }

        let map = XdmMap::from_entries(map_entries);
        let result = XdmValue::from_map(map);

        self.set_last_constructed_value(result.clone());

        Ok(result)
    }

    pub(crate) fn handle_array(
        &mut self,
        members: &[ArrayMemberInstruction],
        context_node: N,
        context_position: usize,
        context_size: usize,
        _builder: &mut dyn OutputBuilder,
    ) -> Result<XdmValue<N>, ExecutionError> {
        let mut array_members: Vec<XdmValue<N>> = Vec::with_capacity(members.len());

        for member in members {
            let value = if let Some(sel) = &member.select {
                self.evaluate_xpath31_xdm(sel, context_node, context_position, context_size)?
            } else if let Some(body) = &member.body {
                let mut body_builder = TextCollector::new();
                self.execute_template(
                    body,
                    context_node,
                    context_position,
                    context_size,
                    &mut body_builder,
                )?;
                XdmValue::from_string(body_builder.into_text())
            } else {
                XdmValue::empty()
            };

            array_members.push(value);
        }

        let array = XdmArray::from_members(array_members);
        let result = XdmValue::from_array(array);

        self.set_last_constructed_value(result.clone());

        Ok(result)
    }

    fn xdm_to_atomic_key(&self, xdm: &XdmValue<N>) -> Result<AtomicValue, ExecutionError> {
        let items = xdm.items();
        if items.is_empty() {
            return Err(ExecutionError::TypeError(
                "Map key cannot be empty sequence".to_string(),
            ));
        }

        match &items[0] {
            petty_xpath31::types::XdmItem::Atomic(a) => Ok(a.clone()),
            petty_xpath31::types::XdmItem::Node(_) => {
                Ok(AtomicValue::String(xdm.to_string_value()))
            }
            _ => Err(ExecutionError::TypeError(
                "Map key must be an atomic value".to_string(),
            )),
        }
    }

    pub(crate) fn set_last_constructed_value(&mut self, value: XdmValue<N>) {
        self.last_constructed_value = Some(value);
    }

    pub(crate) fn handle_where_populated(
        &mut self,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let mut buffering = BufferingOutputBuilder::new(builder);
        buffering.start_buffering();

        self.execute_template(
            body,
            context_node,
            context_position,
            context_size,
            &mut buffering,
        )?;

        buffering.flush_if_populated();
        Ok(())
    }

    pub(crate) fn handle_on_empty(
        &mut self,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        self.execute_template(body, context_node, context_position, context_size, builder)
    }

    pub(crate) fn handle_on_non_empty(
        &mut self,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        self.execute_template(body, context_node, context_position, context_size, builder)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_next_match(
        &mut self,
        _params: &[WithParam3],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let mode = self.current_mode.clone();
        let current_idx = self.current_template_index;

        let rules_for_mode = self
            .stylesheet
            .template_rules
            .get(&mode)
            .cloned()
            .unwrap_or_default();

        let start_idx = current_idx.map(|i| i + 1).unwrap_or(0);

        for (idx, rule) in rules_for_mode.iter().enumerate().skip(start_idx) {
            if self.pattern_matches(&rule.pattern.0, context_node) {
                let prev_idx = self.current_template_index;
                self.current_template_index = Some(idx);

                self.push_scope();
                let result = self.execute_template(
                    &rule.body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                );
                self.pop_scope();

                self.current_template_index = prev_idx;
                return result;
            }
        }

        self.apply_builtin_template(context_node, builder)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_apply_imports(
        &mut self,
        _params: &[WithParam3],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let mode = self.current_mode.clone();

        let rules_for_mode = self
            .stylesheet
            .template_rules
            .get(&mode)
            .cloned()
            .unwrap_or_default();

        for rule in &rules_for_mode {
            if rule.from_import && self.pattern_matches(&rule.pattern.0, context_node) {
                self.push_scope();
                let result = self.execute_template(
                    &rule.body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                );
                self.pop_scope();
                return result;
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_analyze_string(
        &mut self,
        select: &petty_xpath31::Expression,
        regex_str: &str,
        flags: Option<&str>,
        matching_substring: Option<&PreparsedTemplate>,
        non_matching_substring: Option<&PreparsedTemplate>,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let input = self.evaluate_xpath31(select, context_node, context_position, context_size)?;

        let regex_pattern = build_regex_pattern(regex_str, flags)?;
        let re = Regex::new(&regex_pattern).map_err(|e| ExecutionError::DynamicError {
            code: "FORX0002".to_string(),
            message: format!("Invalid regular expression: {}", e),
        })?;

        let mut last_end: usize = 0;
        for mat in re.find_iter(&input) {
            if mat.start() > last_end {
                let non_matching = &input[last_end..mat.start()];
                if let Some(template) = non_matching_substring {
                    let prev_match = self.regex_match.take();
                    let prev_groups = std::mem::take(&mut self.regex_groups);

                    self.regex_match = Some(non_matching.to_string());
                    self.regex_groups = vec![];

                    self.set_variable(
                        "regex:match".to_string(),
                        XdmValue::from_string(non_matching.to_string()),
                    );

                    self.execute_template(
                        template,
                        context_node,
                        context_position,
                        context_size,
                        builder,
                    )?;

                    self.regex_match = prev_match;
                    self.regex_groups = prev_groups;
                }
            }

            if let Some(template) = matching_substring {
                let prev_match = self.regex_match.take();
                let prev_groups = std::mem::take(&mut self.regex_groups);

                let matched_text = mat.as_str().to_string();
                self.regex_match = Some(matched_text.clone());

                if let Some(captures) = re.captures(mat.as_str()) {
                    self.regex_groups = captures
                        .iter()
                        .skip(1)
                        .filter_map(|m: Option<regex::Match<'_>>| m.map(|m| m.as_str().to_string()))
                        .collect();
                } else {
                    self.regex_groups = vec![];
                }

                self.set_variable(
                    "regex:match".to_string(),
                    XdmValue::from_string(matched_text),
                );

                let groups_clone: Vec<_> = self.regex_groups.clone();
                for (i, group) in groups_clone.iter().enumerate() {
                    self.set_variable(
                        format!("::regex-group{}", i + 1),
                        XdmValue::from_string(group.clone()),
                    );
                }

                self.execute_template(
                    template,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;

                self.regex_match = prev_match;
                self.regex_groups = prev_groups;
            }

            last_end = mat.end();
        }

        if last_end < input.len() {
            let non_matching = &input[last_end..];
            if let Some(template) = non_matching_substring {
                let prev_match = self.regex_match.take();
                let prev_groups = std::mem::take(&mut self.regex_groups);

                self.regex_match = Some(non_matching.to_string());
                self.regex_groups = vec![];

                self.set_variable(
                    "regex:match".to_string(),
                    XdmValue::from_string(non_matching.to_string()),
                );

                self.execute_template(
                    template,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;

                self.regex_match = prev_match;
                self.regex_groups = prev_groups;
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_perform_sort(
        &mut self,
        select: Option<&petty_xpath31::Expression>,
        sort_keys: &[SortKey3],
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let items = if let Some(sel) = select {
            self.evaluate_xpath31_xdm(sel, context_node, context_position, context_size)?
        } else {
            let mut body_builder = TextCollector::new();
            self.execute_template(
                body,
                context_node,
                context_position,
                context_size,
                &mut body_builder,
            )?;
            XdmValue::from_string(body_builder.into_text())
        };

        let xdm_items = items.items();
        if xdm_items.is_empty() || sort_keys.is_empty() {
            return Ok(());
        }

        let nodes: Vec<N> = xdm_items
            .iter()
            .filter_map(|item| {
                if let XdmItem::Node(n) = item {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect();

        let sorted = self.sort_nodes(&nodes, sort_keys)?;

        for (i, node) in sorted.iter().enumerate() {
            self.execute_template(body, *node, i + 1, sorted.len(), builder)?;
        }

        Ok(())
    }

    pub(crate) fn sort_nodes(
        &self,
        nodes: &[N],
        sort_keys: &[SortKey3],
    ) -> Result<Vec<N>, ExecutionError> {
        if nodes.is_empty() || sort_keys.is_empty() {
            return Ok(nodes.to_vec());
        }

        let mut indexed: Vec<(usize, N)> = nodes.iter().enumerate().map(|(i, n)| (i, *n)).collect();

        let first_key = &sort_keys[0];
        let node_count = nodes.len();

        indexed.sort_by(|a, b| {
            let a_val = self
                .evaluate_xpath31(&first_key.select, a.1, a.0 + 1, node_count)
                .unwrap_or_default();
            let b_val = self
                .evaluate_xpath31(&first_key.select, b.1, b.0 + 1, node_count)
                .unwrap_or_default();

            let cmp = match first_key.data_type {
                petty_xslt::ast::SortDataType::Number => {
                    let a_num: f64 = a_val.parse().unwrap_or(f64::NAN);
                    let b_num: f64 = b_val.parse().unwrap_or(f64::NAN);
                    a_num
                        .partial_cmp(&b_num)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                _ => a_val.cmp(&b_val),
            };

            match first_key.order {
                petty_xslt::ast::SortOrder::Descending => cmp.reverse(),
                petty_xslt::ast::SortOrder::Ascending => cmp,
            }
        });

        Ok(indexed.into_iter().map(|(_, n)| n).collect())
    }
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_result_document(
        &mut self,
        _format: &Option<String>,
        href: &Option<AttributeValueTemplate>,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        primary_builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let target_href = match href {
            Some(avt) => self.evaluate_avt(avt, context_node, context_position, context_size)?,
            None => String::new(),
        };

        let is_primary = target_href.is_empty() || target_href == "#default";

        if self.active_result_documents.contains(&target_href) {
            return Err(ExecutionError::DynamicError {
                code: "XTDE1500".to_string(),
                message: format!(
                    "Nested xsl:result-document with same href '{}' is not allowed",
                    target_href
                ),
            });
        }

        if is_primary {
            return self.execute_template(
                body,
                context_node,
                context_position,
                context_size,
                primary_builder,
            );
        }

        let sink = self
            .output_sink
            .as_ref()
            .ok_or_else(|| ExecutionError::DynamicError {
                code: "XTDE1480".to_string(),
                message: format!(
                    "xsl:result-document with href='{}' requires an OutputSink. \
                     Call executor.with_output_sink() to enable multi-document output.",
                    target_href
                ),
            })?;

        if sink.has_href(&target_href) {
            return Err(ExecutionError::DynamicError {
                code: "XTDE1490".to_string(),
                message: format!("Duplicate xsl:result-document with href '{}'", target_href),
            });
        }

        let mut doc_output =
            sink.create_output(&target_href)
                .map_err(|e| ExecutionError::DynamicError {
                    code: "XTDE1480".to_string(),
                    message: e.to_string(),
                })?;

        self.active_result_documents.push(target_href.clone());

        let result = self.execute_template(
            body,
            context_node,
            context_position,
            context_size,
            doc_output.builder(),
        );

        self.active_result_documents.pop();

        result?;

        doc_output
            .finish()
            .map_err(|e| ExecutionError::DynamicError {
                code: "XTDE1480".to_string(),
                message: format!("Failed to finish result document '{}': {}", target_href, e),
            })?;

        Ok(())
    }
}

fn build_regex_pattern(regex: &str, flags: Option<&str>) -> Result<String, ExecutionError> {
    let mut pattern = String::new();

    if let Some(f) = flags {
        if f.contains('i') {
            pattern.push_str("(?i)");
        }
        if f.contains('m') {
            pattern.push_str("(?m)");
        }
        if f.contains('s') {
            pattern.push_str("(?s)");
        }
        if f.contains('x') {
            pattern.push_str("(?x)");
        }
    }

    let unescaped = regex.replace("{{", "{").replace("}}", "}");
    pattern.push_str(&unescaped);
    Ok(pattern)
}
