use crate::ast::{Accumulator, AccumulatorPhase};
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::{DataSourceNode, NodeType};
use petty_xslt::output::OutputBuilder;

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub(crate) fn initialize_accumulators(&mut self) -> Result<(), ExecutionError> {
        for (name, acc) in &self.stylesheet.accumulators {
            let initial = self.evaluate_xpath31(&acc.initial_value, self.root_node, 1, 1)?;
            self.accumulator_values.insert(name.clone(), initial);
        }
        Ok(())
    }

    pub(crate) fn process_accumulator_node(
        &mut self,
        node: N,
        context_position: usize,
        context_size: usize,
        phase: AccumulatorPhase,
    ) -> Result<(), ExecutionError> {
        let accumulators: Vec<(String, Accumulator)> = self
            .stylesheet
            .accumulators
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (name, acc) in accumulators {
            let current = self
                .accumulator_values
                .get(&name)
                .cloned()
                .unwrap_or_default();

            self.accumulator_before_values
                .insert(name.clone(), current.clone());

            for rule in &acc.rules {
                if rule.phase == phase && self.pattern_matches_node(&rule.pattern.0, node) {
                    self.set_variable(
                        "value".to_string(),
                        petty_xpath31::types::XdmValue::from_string(current),
                    );

                    let new_value =
                        self.evaluate_xpath31(&rule.select, node, context_position, context_size)?;

                    self.accumulator_values.insert(name.clone(), new_value);
                    break;
                }
            }
        }

        Ok(())
    }

    pub(crate) fn get_accumulator_before(&self, name: &str) -> Option<String> {
        self.accumulator_before_values.get(name).cloned()
    }

    pub(crate) fn get_accumulator_after(&self, name: &str) -> Option<String> {
        self.accumulator_values.get(name).cloned()
    }

    pub(crate) fn handle_accumulator_before(&self, name: &str, builder: &mut dyn OutputBuilder) {
        if let Some(value) = self.get_accumulator_before(name) {
            builder.add_text(&value);
        }
    }

    pub(crate) fn handle_accumulator_after(&self, name: &str, builder: &mut dyn OutputBuilder) {
        if let Some(value) = self.get_accumulator_after(name) {
            builder.add_text(&value);
        }
    }

    fn pattern_matches_node(&self, pattern: &str, node: N) -> bool {
        if pattern.contains('|') {
            return pattern
                .split('|')
                .any(|p| self.pattern_matches_single(p.trim(), node));
        }
        self.pattern_matches_single(pattern, node)
    }

    fn pattern_matches_single(&self, pattern: &str, node: N) -> bool {
        let (base_pattern, predicate) = if let Some(bracket_pos) = pattern.find('[') {
            if let Some(end_pos) = pattern.rfind(']') {
                (
                    &pattern[..bracket_pos],
                    Some(&pattern[bracket_pos + 1..end_pos]),
                )
            } else {
                (pattern, None)
            }
        } else {
            (pattern, None)
        };

        let base_matches = self.pattern_matches_base(base_pattern, node);
        if !base_matches {
            return false;
        }

        if let Some(pred) = predicate {
            return self.predicate_matches(pred.trim(), node);
        }

        true
    }

    fn pattern_matches_base(&self, pattern: &str, node: N) -> bool {
        match node.node_type() {
            NodeType::Root => pattern == "/" || pattern == "/*",
            NodeType::Element => {
                if pattern == "*" || pattern == "node()" {
                    return true;
                }
                if let Some(qname) = node.name() {
                    let name = qname.local_part;
                    pattern == name
                        || pattern == "*"
                        || pattern.ends_with(&format!("/{}", name))
                        || pattern.ends_with("/*")
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

    fn predicate_matches(&self, predicate: &str, node: N) -> bool {
        if let Some(stripped) = predicate.strip_prefix('@') {
            if let Some((attr_name, value)) = stripped.split_once('=') {
                let attr_name = attr_name.trim();
                let value = value.trim().trim_matches('\'').trim_matches('"');
                for attr in node.attributes() {
                    if let Some(qname) = attr.name()
                        && qname.local_part == attr_name
                        && attr.string_value() == value
                    {
                        return true;
                    }
                }
                return false;
            } else {
                let attr_name = stripped.trim();
                for attr in node.attributes() {
                    if let Some(qname) = attr.name()
                        && qname.local_part == attr_name
                    {
                        return true;
                    }
                }
                return false;
            }
        }

        if let Ok(pos) = predicate.parse::<usize>() {
            return pos == 1;
        }

        true
    }
}
