use super::context::StreamedNode;
use petty_xpath31::types::{AtomicValue, XdmItem, XdmValue};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccumulatorPhase {
    Pre,
    Post,
}

#[derive(Debug, Clone)]
pub struct AccumulatorRule {
    pub pattern: String,
    pub phase: AccumulatorPhase,
    pub select: Option<String>,
    pub new_value_expr: Option<String>,
}

impl AccumulatorRule {
    pub fn pre(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            phase: AccumulatorPhase::Pre,
            select: None,
            new_value_expr: None,
        }
    }

    pub fn post(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            phase: AccumulatorPhase::Post,
            select: None,
            new_value_expr: None,
        }
    }

    pub fn with_select(mut self, select: impl Into<String>) -> Self {
        self.select = Some(select.into());
        self
    }

    pub fn with_new_value(mut self, expr: impl Into<String>) -> Self {
        self.new_value_expr = Some(expr.into());
        self
    }

    pub fn matches(&self, node: &StreamedNode) -> bool {
        if self.pattern == "*" {
            return node.is_element();
        }
        if self.pattern == "text()" {
            return node.is_text();
        }
        if let Some(name) = node.local_name() {
            return self.pattern == name;
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct AccumulatorDefinition {
    pub name: String,
    pub initial_value: XdmValue<StreamedNode>,
    pub rules: Vec<AccumulatorRule>,
    pub streamable: bool,
    pub as_type: Option<String>,
}

impl AccumulatorDefinition {
    pub fn new(name: impl Into<String>, initial_value: XdmValue<StreamedNode>) -> Self {
        Self {
            name: name.into(),
            initial_value,
            rules: Vec::new(),
            streamable: true,
            as_type: None,
        }
    }

    pub fn with_rule(mut self, rule: AccumulatorRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn with_type(mut self, as_type: impl Into<String>) -> Self {
        self.as_type = Some(as_type.into());
        self
    }

    pub fn streamable(mut self, streamable: bool) -> Self {
        self.streamable = streamable;
        self
    }
}

#[derive(Debug, Clone)]
struct AccumulatorValue {
    before: XdmValue<StreamedNode>,
    after: XdmValue<StreamedNode>,
}

pub struct AccumulatorRuntime {
    definitions: HashMap<String, AccumulatorDefinition>,
    current_values: HashMap<String, XdmValue<StreamedNode>>,
    value_stack: Vec<HashMap<String, AccumulatorValue>>,
}

impl AccumulatorRuntime {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            current_values: HashMap::new(),
            value_stack: Vec::new(),
        }
    }

    pub fn register(&mut self, definition: AccumulatorDefinition) {
        let initial = definition.initial_value.clone();
        self.current_values.insert(definition.name.clone(), initial);
        self.definitions.insert(definition.name.clone(), definition);
    }

    pub fn get_definition(&self, name: &str) -> Option<&AccumulatorDefinition> {
        self.definitions.get(name)
    }

    pub fn before(&self, name: &str) -> Option<XdmValue<StreamedNode>> {
        if let Some(stack_top) = self.value_stack.last()
            && let Some(value) = stack_top.get(name)
        {
            return Some(value.before.clone());
        }
        self.current_values.get(name).cloned()
    }

    pub fn after(&self, name: &str) -> Option<XdmValue<StreamedNode>> {
        if let Some(stack_top) = self.value_stack.last()
            && let Some(value) = stack_top.get(name)
        {
            return Some(value.after.clone());
        }
        self.current_values.get(name).cloned()
    }

    pub fn current_value(&self, name: &str) -> Option<XdmValue<StreamedNode>> {
        self.current_values.get(name).cloned()
    }

    pub fn set_value(&mut self, name: &str, value: XdmValue<StreamedNode>) {
        self.current_values.insert(name.to_string(), value);
    }

    pub fn push_node(&mut self, node: &StreamedNode) {
        let mut node_values = HashMap::new();

        for (name, def) in &self.definitions {
            let before_value = self
                .current_values
                .get(name)
                .cloned()
                .unwrap_or_else(|| def.initial_value.clone());

            let mut after_value = before_value.clone();
            for rule in &def.rules {
                if rule.phase == AccumulatorPhase::Pre && rule.matches(node) {
                    after_value = self.apply_rule(rule, &before_value, node);
                    break;
                }
            }

            self.current_values
                .insert(name.clone(), after_value.clone());

            node_values.insert(
                name.clone(),
                AccumulatorValue {
                    before: before_value,
                    after: after_value,
                },
            );
        }

        self.value_stack.push(node_values);
    }

    pub fn pop_node(&mut self, node: &StreamedNode) {
        for (name, def) in &self.definitions {
            let current = self
                .current_values
                .get(name)
                .cloned()
                .unwrap_or_else(|| def.initial_value.clone());

            for rule in &def.rules {
                if rule.phase == AccumulatorPhase::Post && rule.matches(node) {
                    let new_value = self.apply_rule(rule, &current, node);
                    self.current_values.insert(name.clone(), new_value.clone());

                    if let Some(stack_top) = self.value_stack.last_mut()
                        && let Some(entry) = stack_top.get_mut(name)
                    {
                        entry.after = new_value;
                    }
                    break;
                }
            }
        }

        self.value_stack.pop();
    }

    fn extract_integer(value: &XdmValue<StreamedNode>) -> Option<i64> {
        let items = value.items();
        if items.len() == 1
            && let XdmItem::Atomic(AtomicValue::Integer(n)) = &items[0]
        {
            return Some(*n);
        }
        None
    }

    fn apply_rule(
        &self,
        rule: &AccumulatorRule,
        current: &XdmValue<StreamedNode>,
        node: &StreamedNode,
    ) -> XdmValue<StreamedNode> {
        if let Some(ref expr) = rule.select {
            if expr == "$value + 1"
                && let Some(n) = Self::extract_integer(current)
            {
                return XdmValue::from_integer(n + 1);
            }
            if expr.starts_with("$value + ")
                && let Some(n) = Self::extract_integer(current)
                && let Some(node_val) = self.extract_node_value(node, expr)
            {
                return XdmValue::from_integer(n + node_val);
            }
            if expr == "($value, .)" {
                let items = current.items();
                let mut new_items: Vec<XdmItem<StreamedNode>> = items.to_vec();
                new_items.push(XdmItem::Atomic(AtomicValue::String(node.string_value())));
                return XdmValue::from_items(new_items);
            }
        }

        current.clone()
    }

    fn extract_node_value(&self, node: &StreamedNode, expr: &str) -> Option<i64> {
        if expr.contains("@amount") {
            for attr in &node.attributes {
                if attr.name.local_name == "amount" {
                    return attr.value.parse().ok();
                }
            }
        }
        if expr.contains("@value") {
            for attr in &node.attributes {
                if attr.name.local_name == "value" {
                    return attr.value.parse().ok();
                }
            }
        }
        if expr.contains("xs:integer(.)") || expr.contains("number(.)") {
            return node.string_value().trim().parse().ok();
        }
        None
    }

    pub fn reset(&mut self) {
        for (name, def) in &self.definitions {
            self.current_values
                .insert(name.clone(), def.initial_value.clone());
        }
        self.value_stack.clear();
    }
}

impl Default for AccumulatorRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::event_model::{Attribute, QName};

    #[test]
    fn test_accumulator_definition() {
        let acc = AccumulatorDefinition::new("counter", XdmValue::from_integer(0))
            .with_rule(AccumulatorRule::pre("item").with_select("$value + 1"))
            .with_type("xs:integer");

        assert_eq!(acc.name, "counter");
        assert_eq!(acc.rules.len(), 1);
        assert!(acc.streamable);
    }

    #[test]
    fn test_accumulator_rule_matching() {
        let rule = AccumulatorRule::pre("item");

        let item_node = StreamedNode::element(QName::new("item"), vec![], 1, 1);
        let other_node = StreamedNode::element(QName::new("other"), vec![], 1, 1);
        let text_node = StreamedNode::text("hello".to_string(), 1, 1);

        assert!(rule.matches(&item_node));
        assert!(!rule.matches(&other_node));
        assert!(!rule.matches(&text_node));
    }

    #[test]
    fn test_accumulator_runtime_counter() {
        let mut runtime = AccumulatorRuntime::new();

        runtime.register(
            AccumulatorDefinition::new("counter", XdmValue::from_integer(0))
                .with_rule(AccumulatorRule::pre("item").with_select("$value + 1")),
        );

        let item1 = StreamedNode::element(QName::new("item"), vec![], 1, 1);
        let item2 = StreamedNode::element(QName::new("item"), vec![], 1, 2);

        runtime.push_node(&item1);
        let after_val = runtime.after("counter").unwrap();
        assert_eq!(AccumulatorRuntime::extract_integer(&after_val), Some(1));

        runtime.pop_node(&item1);

        runtime.push_node(&item2);
        let after_val = runtime.after("counter").unwrap();
        assert_eq!(AccumulatorRuntime::extract_integer(&after_val), Some(2));
    }

    #[test]
    fn test_accumulator_sum_with_attribute() {
        let mut runtime = AccumulatorRuntime::new();

        runtime.register(
            AccumulatorDefinition::new("total", XdmValue::from_integer(0))
                .with_rule(AccumulatorRule::pre("item").with_select("$value + @amount")),
        );

        let item1 = StreamedNode::element(
            QName::new("item"),
            vec![Attribute {
                name: QName::new("amount"),
                value: "10".to_string(),
            }],
            1,
            1,
        );

        let item2 = StreamedNode::element(
            QName::new("item"),
            vec![Attribute {
                name: QName::new("amount"),
                value: "25".to_string(),
            }],
            1,
            2,
        );

        runtime.push_node(&item1);
        runtime.pop_node(&item1);
        runtime.push_node(&item2);

        let after_val = runtime.after("total").unwrap();
        assert_eq!(AccumulatorRuntime::extract_integer(&after_val), Some(35));
    }

    #[test]
    fn test_accumulator_before_after() {
        let mut runtime = AccumulatorRuntime::new();

        runtime.register(
            AccumulatorDefinition::new("counter", XdmValue::from_integer(0))
                .with_rule(AccumulatorRule::pre("*").with_select("$value + 1")),
        );

        let node = StreamedNode::element(QName::new("item"), vec![], 1, 1);

        let before_val = runtime.before("counter").unwrap();
        assert_eq!(AccumulatorRuntime::extract_integer(&before_val), Some(0));

        runtime.push_node(&node);

        let before_val = runtime.before("counter").unwrap();
        let after_val = runtime.after("counter").unwrap();
        assert_eq!(AccumulatorRuntime::extract_integer(&before_val), Some(0));
        assert_eq!(AccumulatorRuntime::extract_integer(&after_val), Some(1));
    }

    #[test]
    fn test_accumulator_wildcard_match() {
        let rule = AccumulatorRule::pre("*");

        let elem = StreamedNode::element(QName::new("anything"), vec![], 1, 1);
        let text = StreamedNode::text("text".to_string(), 1, 1);

        assert!(rule.matches(&elem));
        assert!(!rule.matches(&text));
    }

    #[test]
    fn test_accumulator_text_match() {
        let rule = AccumulatorRule::pre("text()");

        let elem = StreamedNode::element(QName::new("elem"), vec![], 1, 1);
        let text = StreamedNode::text("hello".to_string(), 1, 1);

        assert!(!rule.matches(&elem));
        assert!(rule.matches(&text));
    }

    #[test]
    fn test_accumulator_reset() {
        let mut runtime = AccumulatorRuntime::new();

        runtime.register(
            AccumulatorDefinition::new("counter", XdmValue::from_integer(0))
                .with_rule(AccumulatorRule::pre("item").with_select("$value + 1")),
        );

        let node = StreamedNode::element(QName::new("item"), vec![], 1, 1);
        runtime.push_node(&node);
        runtime.pop_node(&node);

        let curr_val = runtime.current_value("counter").unwrap();
        assert_eq!(AccumulatorRuntime::extract_integer(&curr_val), Some(1));

        runtime.reset();
        let curr_val = runtime.current_value("counter").unwrap();
        assert_eq!(AccumulatorRuntime::extract_integer(&curr_val), Some(0));
    }
}
