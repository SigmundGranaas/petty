//! Literal output execution: content tags, xsl:number, xsl:copy, xsl:copy-of.

#![allow(clippy::too_many_arguments)]

use crate::ast::{Avt3, NumberLevel, PreparsedTemplate, ShadowAttribute};
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::Expression;
use petty_xslt::ast::{AttributeValueTemplate, PreparsedStyles};
use petty_xslt::output::OutputBuilder;
use std::collections::HashMap;

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_content_tag(
        &mut self,
        tag_name: &[u8],
        styles: &PreparsedStyles,
        attrs: &HashMap<String, Avt3>,
        shadow_attrs: &[ShadowAttribute],
        use_attribute_sets: &[String],
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let _evaluated_attrs: HashMap<String, String> = attrs
            .iter()
            .map(|(name, avt)| {
                let value =
                    self.evaluate_avt3(avt, context_node, context_position, context_size)?;
                Ok((name.clone(), value))
            })
            .collect::<Result<HashMap<_, _>, ExecutionError>>()?;

        self.execute_start_tag(tag_name, styles, builder);
        self.emit_shadow_attributes(
            shadow_attrs,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
        self.expand_attribute_sets(
            use_attribute_sets,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
        self.execute_template(body, context_node, context_position, context_size, builder)?;
        self.execute_end_tag(tag_name, builder);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_empty_tag(
        &mut self,
        tag_name: &[u8],
        styles: &PreparsedStyles,
        attrs: &HashMap<String, Avt3>,
        shadow_attrs: &[ShadowAttribute],
        use_attribute_sets: &[String],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let _evaluated_attrs: HashMap<String, String> = attrs
            .iter()
            .map(|(name, avt)| {
                let value =
                    self.evaluate_avt3(avt, context_node, context_position, context_size)?;
                Ok((name.clone(), value))
            })
            .collect::<Result<HashMap<_, _>, ExecutionError>>()?;

        self.execute_start_tag(tag_name, styles, builder);
        self.emit_shadow_attributes(
            shadow_attrs,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
        self.expand_attribute_sets(
            use_attribute_sets,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
        self.execute_end_tag(tag_name, builder);

        Ok(())
    }

    fn emit_shadow_attributes(
        &mut self,
        shadow_attrs: &[ShadowAttribute],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        for shadow in shadow_attrs {
            let attr_name =
                self.evaluate_avt3(&shadow.name, context_node, context_position, context_size)?;
            let attr_value =
                self.evaluate_avt3(&shadow.value, context_node, context_position, context_size)?;
            builder.set_attribute(&attr_name, &attr_value);
        }
        Ok(())
    }

    fn expand_attribute_sets(
        &mut self,
        set_names: &[String],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        for set_name in set_names {
            self.expand_single_attribute_set(
                set_name,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
        }
        Ok(())
    }

    fn expand_single_attribute_set(
        &mut self,
        set_name: &str,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let attr_set = self.stylesheet.attribute_sets.get(set_name).cloned();
        if let Some(attr_set) = attr_set {
            self.expand_attribute_sets(
                &attr_set.use_attribute_sets,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
            self.execute_template(
                &attr_set.attributes,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
        }
        Ok(())
    }

    pub(crate) fn handle_copy(
        &mut self,
        styles: &PreparsedStyles,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        builder.start_block(styles);
        self.execute_template(body, context_node, context_position, context_size, builder)?;
        builder.end_block();
        Ok(())
    }

    pub(crate) fn handle_attribute(
        &mut self,
        name: &AttributeValueTemplate,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let _attr_name = self.evaluate_avt(name, context_node, context_position, context_size)?;
        self.execute_template(body, context_node, context_position, context_size, builder)?;
        Ok(())
    }

    pub(crate) fn handle_element(
        &mut self,
        name: &AttributeValueTemplate,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let element_name = self.evaluate_avt(name, context_node, context_position, context_size)?;
        let styles = PreparsedStyles::default();

        self.execute_start_tag(element_name.as_bytes(), &styles, builder);
        self.execute_template(body, context_node, context_position, context_size, builder)?;
        self.execute_end_tag(element_name.as_bytes(), builder);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_number(
        &mut self,
        level: &NumberLevel,
        count: &Option<String>,
        from: &Option<String>,
        value: &Option<Expression>,
        format: &Avt3,
        _lang: &Option<String>,
        _letter_value: &Option<String>,
        grouping_separator: &Option<String>,
        grouping_size: &Option<u32>,
        _ordinal: &Option<String>,
        select: &Option<Expression>,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let numbers = if let Some(val_expr) = value {
            let result =
                self.evaluate_xpath31_xdm(val_expr, context_node, context_position, context_size)?;
            vec![result.to_double() as usize]
        } else {
            let target_node = if let Some(sel) = select {
                let nodes =
                    self.evaluate_xpath31_nodes(sel, context_node, context_position, context_size)?;
                nodes.into_iter().next().unwrap_or(context_node)
            } else {
                context_node
            };

            self.compute_number_value(target_node, level, count.as_deref(), from.as_deref())?
        };

        let format_str =
            self.evaluate_avt3(format, context_node, context_position, context_size)?;
        let formatted =
            self.format_numbers(&numbers, &format_str, grouping_separator, grouping_size);

        builder.add_text(&formatted);
        Ok(())
    }

    fn compute_number_value(
        &self,
        node: N,
        level: &NumberLevel,
        count: Option<&str>,
        from: Option<&str>,
    ) -> Result<Vec<usize>, ExecutionError> {
        match level {
            NumberLevel::Single => {
                let num = self.count_preceding_siblings_for_number(node, count, from)?;
                Ok(vec![num])
            }
            NumberLevel::Multiple => self.count_ancestors_for_number(node, count, from),
            NumberLevel::Any => {
                let num = self.count_preceding_any_for_number(node, count, from)?;
                Ok(vec![num])
            }
        }
    }

    fn count_preceding_siblings_for_number(
        &self,
        node: N,
        count: Option<&str>,
        _from: Option<&str>,
    ) -> Result<usize, ExecutionError> {
        let node_name = node.name().map(|q| q.local_part).unwrap_or("");
        let count_pattern = count.unwrap_or(node_name);

        let mut position = 1;

        if let Some(parent) = node.parent() {
            for sibling in parent.children() {
                if sibling == node {
                    break;
                }
                if self.node_matches_number_pattern(sibling, count_pattern) {
                    position += 1;
                }
            }
        }

        Ok(position)
    }

    fn count_ancestors_for_number(
        &self,
        node: N,
        count: Option<&str>,
        _from: Option<&str>,
    ) -> Result<Vec<usize>, ExecutionError> {
        let mut numbers = Vec::new();
        let node_name = node.name().map(|q| q.local_part).unwrap_or("");
        let count_pattern = count.unwrap_or(node_name);

        let mut current = Some(node);
        while let Some(n) = current {
            if self.node_matches_number_pattern(n, count_pattern) {
                let pos = self.count_preceding_siblings_for_number(n, Some(count_pattern), None)?;
                numbers.push(pos);
            }
            current = n.parent();
        }

        numbers.reverse();
        Ok(numbers)
    }

    fn count_preceding_any_for_number(
        &self,
        node: N,
        count: Option<&str>,
        _from: Option<&str>,
    ) -> Result<usize, ExecutionError> {
        let node_name = node.name().map(|q| q.local_part).unwrap_or("");
        let count_pattern = count.unwrap_or(node_name);

        let mut position = 0;
        let root = self.find_document_root_for_number(node);

        self.count_in_document_order(root, node, count_pattern, &mut position);

        Ok(position)
    }

    fn count_in_document_order(
        &self,
        current: N,
        target: N,
        pattern: &str,
        count: &mut usize,
    ) -> bool {
        if current == target {
            if self.node_matches_number_pattern(current, pattern) {
                *count += 1;
            }
            return true;
        }

        if self.node_matches_number_pattern(current, pattern) {
            *count += 1;
        }

        for child in current.children() {
            if self.count_in_document_order(child, target, pattern, count) {
                return true;
            }
        }

        false
    }

    fn find_document_root_for_number(&self, node: N) -> N {
        let mut current = node;
        while let Some(parent) = current.parent() {
            current = parent;
        }
        current
    }

    fn node_matches_number_pattern(&self, node: N, pattern: &str) -> bool {
        if pattern == "*" {
            return node.node_type() == petty_xpath1::datasource::NodeType::Element;
        }

        if let Some(name) = node.name() {
            name.local_part == pattern
        } else {
            false
        }
    }

    fn format_numbers(
        &self,
        numbers: &[usize],
        format: &str,
        grouping_separator: &Option<String>,
        grouping_size: &Option<u32>,
    ) -> String {
        if numbers.is_empty() {
            return String::new();
        }

        let format_tokens = self.parse_format_string(format);
        let mut result = String::new();

        for (i, &num) in numbers.iter().enumerate() {
            if i > 0 {
                if let Some(sep) = format_tokens.get(i * 2 - 1) {
                    result.push_str(sep);
                } else {
                    result.push('.');
                }
            }

            let fmt = format_tokens
                .get(i * 2)
                .or_else(|| format_tokens.first())
                .map(|s| s.as_str())
                .unwrap_or("1");

            let formatted = self.format_single_number(num, fmt, grouping_separator, grouping_size);
            result.push_str(&formatted);
        }

        result
    }

    fn parse_format_string(&self, format: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_format = false;

        for ch in format.chars() {
            let is_format_char = ch.is_ascii_digit()
                || ch.is_ascii_alphabetic()
                || ch == 'i'
                || ch == 'I'
                || ch == 'a'
                || ch == 'A'
                || ch == 'w'
                || ch == 'W';

            if is_format_char {
                if !in_format && !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
                in_format = true;
                current.push(ch);
            } else {
                if in_format && !current.is_empty() {
                    tokens.push(current);
                    current = String::new();
                }
                in_format = false;
                current.push(ch);
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        if tokens.is_empty() {
            tokens.push("1".to_string());
        }

        tokens
    }

    fn format_single_number(
        &self,
        num: usize,
        format: &str,
        grouping_separator: &Option<String>,
        grouping_size: &Option<u32>,
    ) -> String {
        let formatted = match format {
            "1" | "" => num.to_string(),
            "01" => format!("{:02}", num),
            "001" => format!("{:03}", num),
            "a" => self.number_to_alpha(num, false),
            "A" => self.number_to_alpha(num, true),
            "i" => self.number_to_roman(num, false),
            "I" => self.number_to_roman(num, true),
            "w" => self.number_to_words(num, false),
            "W" => self.number_to_words(num, true),
            "Ww" => self.number_to_words(num, false),
            other => {
                if other.chars().all(|c| c == '0' || c == '1') {
                    let width = other.len();
                    format!("{:0width$}", num, width = width)
                } else {
                    num.to_string()
                }
            }
        };

        if let (Some(sep), Some(size)) = (grouping_separator, grouping_size)
            && *size > 0
            && format
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            return self.apply_grouping(&formatted, sep, *size as usize);
        }

        formatted
    }

    fn apply_grouping(&self, num_str: &str, separator: &str, size: usize) -> String {
        let chars: Vec<char> = num_str.chars().collect();
        let mut result = String::new();

        for (i, ch) in chars.iter().enumerate() {
            if i > 0 && (chars.len() - i).is_multiple_of(size) {
                result.push_str(separator);
            }
            result.push(*ch);
        }

        result
    }

    fn number_to_alpha(&self, num: usize, uppercase: bool) -> String {
        if num == 0 {
            return String::new();
        }

        let mut result = String::new();
        let mut n = num;

        while n > 0 {
            n -= 1;
            let ch = if uppercase {
                (b'A' + (n % 26) as u8) as char
            } else {
                (b'a' + (n % 26) as u8) as char
            };
            result.insert(0, ch);
            n /= 26;
        }

        result
    }

    fn number_to_roman(&self, num: usize, uppercase: bool) -> String {
        if num == 0 || num > 3999 {
            return num.to_string();
        }

        let values = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
        let numerals = [
            "m", "cm", "d", "cd", "c", "xc", "l", "xl", "x", "ix", "v", "iv", "i",
        ];

        let mut result = String::new();
        let mut n = num;

        for (i, &val) in values.iter().enumerate() {
            while n >= val {
                result.push_str(numerals[i]);
                n -= val;
            }
        }

        if uppercase {
            result.to_uppercase()
        } else {
            result
        }
    }

    fn number_to_words(&self, num: usize, uppercase: bool) -> String {
        let words = match num {
            0 => "zero",
            1 => "one",
            2 => "two",
            3 => "three",
            4 => "four",
            5 => "five",
            6 => "six",
            7 => "seven",
            8 => "eight",
            9 => "nine",
            10 => "ten",
            11 => "eleven",
            12 => "twelve",
            13 => "thirteen",
            14 => "fourteen",
            15 => "fifteen",
            16 => "sixteen",
            17 => "seventeen",
            18 => "eighteen",
            19 => "nineteen",
            20 => "twenty",
            _ => return num.to_string(),
        };

        if uppercase {
            let mut chars = words.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        } else {
            words.to_string()
        }
    }
}
