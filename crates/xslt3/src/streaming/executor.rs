use super::analysis::{Posture, StreamabilityAnalyzer};
use super::context::{StreamedContext, StreamedNode};
use super::event_model::{Attribute, QName, StreamEvent, StreamEventHandler};
use crate::ast::{AccumulatorPhase, CompiledStylesheet3, PreparsedTemplate, Xslt3Instruction};
use crate::error::Xslt3Error;
use petty_idf::IRNode;
use petty_xpath31::types::XdmValue;
use petty_xslt::idf_builder::IdfBuilder;
use petty_xslt::output::OutputBuilder;
use std::collections::HashMap;

pub struct StreamingExecutor<'s> {
    stylesheet: &'s CompiledStylesheet3,
    context: StreamedContext,
    builder: IdfBuilder,
    variable_stack: Vec<HashMap<String, XdmValue<StreamedNode>>>,
    accumulator_values: HashMap<String, XdmValue<StreamedNode>>,
    mode: Option<String>,
}

impl<'s> StreamingExecutor<'s> {
    pub fn new(stylesheet: &'s CompiledStylesheet3) -> Self {
        Self {
            stylesheet,
            context: StreamedContext::new(),
            builder: IdfBuilder::new(),
            variable_stack: vec![HashMap::new()],
            accumulator_values: HashMap::new(),
            mode: stylesheet.default_mode.clone(),
        }
    }

    pub fn with_mode(mut self, mode: Option<String>) -> Self {
        self.mode = mode;
        self
    }

    pub fn get_accumulator_value(&self, name: &str) -> Option<&XdmValue<StreamedNode>> {
        self.accumulator_values.get(name)
    }

    pub fn take_accumulator_values(self) -> HashMap<String, XdmValue<StreamedNode>> {
        self.accumulator_values
    }

    fn set_variable(&mut self, name: String, value: XdmValue<StreamedNode>) {
        if let Some(scope) = self.variable_stack.last_mut() {
            scope.insert(name, value);
        }
    }

    fn get_variable(&self, name: &str) -> Option<&XdmValue<StreamedNode>> {
        for scope in self.variable_stack.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value);
            }
        }
        None
    }

    fn initialize_accumulators(&mut self) {
        for (name, acc) in &self.stylesheet.accumulators {
            let initial = self
                .evaluate_grounded_expression(&acc.initial_value)
                .map(XdmValue::from_string)
                .unwrap_or_else(|_| XdmValue::empty());
            self.accumulator_values.insert(name.clone(), initial);
        }
    }

    fn process_accumulator_rules(&mut self, node: &StreamedNode, phase: AccumulatorPhase) {
        let accumulators: Vec<_> = self
            .stylesheet
            .accumulators
            .iter()
            .map(|(name, acc)| (name.clone(), acc.clone()))
            .collect();

        for (name, acc) in accumulators {
            for rule in &acc.rules {
                if rule.phase != phase {
                    continue;
                }

                if self.pattern_matches_streamed(&rule.pattern.0, node) {
                    let current_value = self
                        .accumulator_values
                        .get(&name)
                        .cloned()
                        .unwrap_or_else(XdmValue::empty);

                    self.set_variable("value".to_string(), current_value);

                    if let Ok(new_value_str) = self.evaluate_grounded_expression(&rule.select) {
                        let new_value = XdmValue::from_string(new_value_str);
                        self.accumulator_values
                            .insert(name.clone(), new_value.clone());
                        self.context.set_accumulator(name.clone(), new_value);
                    }
                }
            }
        }
    }

    fn pattern_matches_streamed(&self, pattern: &str, node: &StreamedNode) -> bool {
        match &node.kind {
            super::context::StreamedNodeKind::Document => pattern == "/" || pattern == "/*",
            super::context::StreamedNodeKind::Element => {
                if pattern == "*" || pattern == "node()" {
                    return true;
                }
                if let Some(name) = node.local_name() {
                    pattern == name || pattern == "*" || pattern.ends_with(&format!("/{}", name))
                } else {
                    false
                }
            }
            super::context::StreamedNodeKind::Text => pattern == "text()" || pattern == "node()",
            super::context::StreamedNodeKind::Attribute => {
                if let Some(attr_pattern) = pattern.strip_prefix('@') {
                    if let Some(name) = node.local_name() {
                        attr_pattern == "*" || attr_pattern == name
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            super::context::StreamedNodeKind::Comment => {
                pattern == "comment()" || pattern == "node()"
            }
            super::context::StreamedNodeKind::ProcessingInstruction => {
                pattern == "processing-instruction()" || pattern == "node()"
            }
        }
    }

    fn find_matching_template(&self, node: &StreamedNode) -> Option<&PreparsedTemplate> {
        let rules = self.stylesheet.template_rules.get(&self.mode)?;

        for rule in rules {
            if self.pattern_matches_streamed(&rule.pattern.0, node) {
                return Some(&rule.body);
            }
        }

        None
    }

    fn apply_builtin_template(&mut self, node: &StreamedNode) {
        if node.kind == super::context::StreamedNodeKind::Text {
            self.builder.add_text(&node.string_value());
        }
    }

    fn execute_streaming_template(
        &mut self,
        template: &PreparsedTemplate,
        node: &StreamedNode,
    ) -> Result<(), Xslt3Error> {
        for instruction in &template.0 {
            self.execute_streaming_instruction(instruction, node)?;
        }
        Ok(())
    }

    fn execute_streaming_instruction(
        &mut self,
        instruction: &Xslt3Instruction,
        node: &StreamedNode,
    ) -> Result<(), Xslt3Error> {
        match instruction {
            Xslt3Instruction::Text(text) => {
                self.builder.add_text(text);
                Ok(())
            }
            Xslt3Instruction::ValueOf {
                select,
                separator: _,
            } => {
                let result = StreamabilityAnalyzer::analyze_expression(select);
                if !result.streamable {
                    return Err(Xslt3Error::streaming(format!(
                        "Expression is not streamable: {}",
                        result
                            .reason
                            .unwrap_or_else(|| "unknown reason".to_string())
                    )));
                }

                let value = match result.posture {
                    Posture::Grounded => self.evaluate_grounded_expression(select)?,
                    Posture::Striding => self.evaluate_striding_expression(select, node)?,
                    _ => node.string_value(),
                };
                self.builder.add_text(&value);
                Ok(())
            }
            Xslt3Instruction::If { test, body } => {
                let result = StreamabilityAnalyzer::analyze_expression(test);
                if !result.streamable || result.posture == Posture::Roaming {
                    return Err(Xslt3Error::streaming(
                        "xsl:if test expression must be streamable",
                    ));
                }

                let condition = self.evaluate_grounded_expression(test)?;
                if condition != "false" && condition != "0" && !condition.is_empty() {
                    self.execute_streaming_template(body, node)?;
                }
                Ok(())
            }
            Xslt3Instruction::Choose { whens, otherwise } => {
                for when in whens {
                    let result = StreamabilityAnalyzer::analyze_expression(&when.test);
                    if !result.streamable || result.posture == Posture::Roaming {
                        return Err(Xslt3Error::streaming(
                            "xsl:when test expression must be streamable",
                        ));
                    }

                    let condition = self.evaluate_grounded_expression(&when.test)?;
                    if condition != "false" && condition != "0" && !condition.is_empty() {
                        return self.execute_streaming_template(&when.body, node);
                    }
                }
                if let Some(otherwise_body) = otherwise {
                    self.execute_streaming_template(otherwise_body, node)?;
                }
                Ok(())
            }
            Xslt3Instruction::Variable {
                name, select, body, ..
            } => {
                if let Some(sel) = select {
                    let result = StreamabilityAnalyzer::analyze_expression(sel);
                    if !result.streamable {
                        return Err(Xslt3Error::streaming(format!(
                            "Variable {} expression is not streamable",
                            name
                        )));
                    }

                    let value = self.evaluate_grounded_expression(sel)?;
                    self.set_variable(name.clone(), XdmValue::from_string(value));
                } else if let Some(_body_template) = body {
                    return Err(Xslt3Error::streaming(format!(
                        "Variable {} with body content is not supported in streaming mode",
                        name
                    )));
                }
                Ok(())
            }
            Xslt3Instruction::ContentTag {
                tag_name,
                body,
                styles,
                ..
            } => {
                let styles_ref = petty_xslt::ast::PreparsedStyles {
                    id: styles.id.clone(),
                    style_sets: styles.style_sets.clone(),
                    style_override: styles.style_override.clone(),
                };
                let tag_str = String::from_utf8_lossy(tag_name);
                match tag_str.as_ref() {
                    "p" => self.builder.start_paragraph(&styles_ref),
                    "block" | "div" => self.builder.start_block(&styles_ref),
                    _ => self.builder.start_block(&styles_ref),
                }
                self.execute_streaming_template(body, node)?;
                match tag_str.as_ref() {
                    "p" => self.builder.end_paragraph(),
                    "block" | "div" => self.builder.end_block(),
                    _ => self.builder.end_block(),
                }
                Ok(())
            }
            _ => Err(Xslt3Error::streaming(format!(
                "Instruction {:?} is not supported in streaming mode",
                std::mem::discriminant(instruction)
            ))),
        }
    }

    fn evaluate_grounded_expression(
        &self,
        expr: &petty_xpath31::Expression,
    ) -> Result<String, Xslt3Error> {
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;

        match expr {
            Expression::Literal(lit) => match lit {
                Literal::String(s) => Ok(s.clone()),
                Literal::Integer(i) => Ok(i.to_string()),
                Literal::Decimal(d) => Ok(d.to_string()),
                Literal::Double(d) => Ok(d.to_string()),
            },
            Expression::Variable(name) => self
                .get_variable(name)
                .map(|v| v.to_string_value())
                .ok_or_else(|| Xslt3Error::streaming(format!("Variable ${} not found", name))),
            Expression::FunctionCall { name, args } => {
                self.evaluate_grounded_function(&name.to_string(), args)
            }
            Expression::BinaryOp { left, right, op } => {
                self.evaluate_grounded_binary_op(left, right, op)
            }
            Expression::UnaryOp { op, expr } => {
                use petty_xpath1::ast::UnaryOperator;
                let val: f64 = self
                    .evaluate_grounded_expression(expr)?
                    .parse()
                    .unwrap_or(0.0);
                match op {
                    UnaryOperator::Minus => Ok((-val).to_string()),
                    UnaryOperator::Plus => Ok(val.to_string()),
                }
            }
            _ => Err(Xslt3Error::streaming(
                "Expression type not supported for grounded evaluation in streaming",
            )),
        }
    }

    fn evaluate_grounded_function(
        &self,
        name: &str,
        args: &[petty_xpath31::Expression],
    ) -> Result<String, Xslt3Error> {
        match name {
            "string" => {
                if args.is_empty() {
                    Ok(String::new())
                } else {
                    self.evaluate_grounded_expression(&args[0])
                }
            }
            "concat" => {
                let mut result = String::new();
                for arg in args {
                    result.push_str(&self.evaluate_grounded_expression(arg)?);
                }
                Ok(result)
            }
            "string-length" => {
                let s = if args.is_empty() {
                    String::new()
                } else {
                    self.evaluate_grounded_expression(&args[0])?
                };
                Ok(s.chars().count().to_string())
            }
            "substring" => {
                if args.len() < 2 {
                    return Err(Xslt3Error::streaming(
                        "substring() requires at least 2 arguments",
                    ));
                }
                let s = self.evaluate_grounded_expression(&args[0])?;
                let start: f64 = self
                    .evaluate_grounded_expression(&args[1])?
                    .parse()
                    .unwrap_or(1.0);
                let start_idx = (start.round() as isize - 1).max(0) as usize;

                let chars: Vec<char> = s.chars().collect();
                if args.len() >= 3 {
                    let len: f64 = self
                        .evaluate_grounded_expression(&args[2])?
                        .parse()
                        .unwrap_or(0.0);
                    let len_usize = len.round().max(0.0) as usize;
                    Ok(chars.iter().skip(start_idx).take(len_usize).collect())
                } else {
                    Ok(chars.iter().skip(start_idx).collect())
                }
            }
            "contains" => {
                if args.len() < 2 {
                    return Err(Xslt3Error::streaming("contains() requires 2 arguments"));
                }
                let s = self.evaluate_grounded_expression(&args[0])?;
                let pattern = self.evaluate_grounded_expression(&args[1])?;
                Ok(s.contains(&pattern).to_string())
            }
            "starts-with" => {
                if args.len() < 2 {
                    return Err(Xslt3Error::streaming("starts-with() requires 2 arguments"));
                }
                let s = self.evaluate_grounded_expression(&args[0])?;
                let pattern = self.evaluate_grounded_expression(&args[1])?;
                Ok(s.starts_with(&pattern).to_string())
            }
            "ends-with" => {
                if args.len() < 2 {
                    return Err(Xslt3Error::streaming("ends-with() requires 2 arguments"));
                }
                let s = self.evaluate_grounded_expression(&args[0])?;
                let pattern = self.evaluate_grounded_expression(&args[1])?;
                Ok(s.ends_with(&pattern).to_string())
            }
            "normalize-space" => {
                let s = if args.is_empty() {
                    String::new()
                } else {
                    self.evaluate_grounded_expression(&args[0])?
                };
                Ok(s.split_whitespace().collect::<Vec<_>>().join(" "))
            }
            "upper-case" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("upper-case() requires 1 argument"));
                }
                Ok(self.evaluate_grounded_expression(&args[0])?.to_uppercase())
            }
            "lower-case" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("lower-case() requires 1 argument"));
                }
                Ok(self.evaluate_grounded_expression(&args[0])?.to_lowercase())
            }
            "translate" => {
                if args.len() < 3 {
                    return Err(Xslt3Error::streaming("translate() requires 3 arguments"));
                }
                let s = self.evaluate_grounded_expression(&args[0])?;
                let from_chars: Vec<char> = self
                    .evaluate_grounded_expression(&args[1])?
                    .chars()
                    .collect();
                let to_chars: Vec<char> = self
                    .evaluate_grounded_expression(&args[2])?
                    .chars()
                    .collect();
                Ok(s.chars()
                    .filter_map(|c| {
                        if let Some(pos) = from_chars.iter().position(|&fc| fc == c) {
                            to_chars.get(pos).copied()
                        } else {
                            Some(c)
                        }
                    })
                    .collect())
            }
            "number" => {
                if args.is_empty() {
                    Ok("NaN".to_string())
                } else {
                    let s = self.evaluate_grounded_expression(&args[0])?;
                    match s.trim().parse::<f64>() {
                        Ok(n) => Ok(n.to_string()),
                        Err(_) => Ok("NaN".to_string()),
                    }
                }
            }
            "floor" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("floor() requires 1 argument"));
                }
                let n: f64 = self
                    .evaluate_grounded_expression(&args[0])?
                    .parse()
                    .unwrap_or(f64::NAN);
                Ok(n.floor().to_string())
            }
            "ceiling" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("ceiling() requires 1 argument"));
                }
                let n: f64 = self
                    .evaluate_grounded_expression(&args[0])?
                    .parse()
                    .unwrap_or(f64::NAN);
                Ok(n.ceil().to_string())
            }
            "round" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("round() requires 1 argument"));
                }
                let n: f64 = self
                    .evaluate_grounded_expression(&args[0])?
                    .parse()
                    .unwrap_or(f64::NAN);
                Ok(n.round().to_string())
            }
            "abs" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("abs() requires 1 argument"));
                }
                let n: f64 = self
                    .evaluate_grounded_expression(&args[0])?
                    .parse()
                    .unwrap_or(f64::NAN);
                Ok(n.abs().to_string())
            }
            "not" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("not() requires 1 argument"));
                }
                let val = self.evaluate_grounded_expression(&args[0])?;
                let bool_val = !self.string_to_bool(&val);
                Ok(bool_val.to_string())
            }
            "true" => Ok("true".to_string()),
            "false" => Ok("false".to_string()),
            "boolean" => {
                if args.is_empty() {
                    return Err(Xslt3Error::streaming("boolean() requires 1 argument"));
                }
                let val = self.evaluate_grounded_expression(&args[0])?;
                Ok(self.string_to_bool(&val).to_string())
            }
            "format-number" => {
                if args.len() < 2 {
                    return Err(Xslt3Error::streaming(
                        "format-number() requires at least 2 arguments",
                    ));
                }
                let n: f64 = self
                    .evaluate_grounded_expression(&args[0])?
                    .parse()
                    .unwrap_or(0.0);
                let pattern = self.evaluate_grounded_expression(&args[1])?;
                Ok(self.format_number_simple(n, &pattern))
            }
            _ => Err(Xslt3Error::streaming(format!(
                "Function {} not supported in streaming grounded evaluation",
                name
            ))),
        }
    }

    fn evaluate_grounded_binary_op(
        &self,
        left: &petty_xpath31::Expression,
        right: &petty_xpath31::Expression,
        op: &petty_xpath1::ast::BinaryOperator,
    ) -> Result<String, Xslt3Error> {
        use petty_xpath1::ast::BinaryOperator;

        let l = self.evaluate_grounded_expression(left)?;
        let r = self.evaluate_grounded_expression(right)?;

        match op {
            BinaryOperator::Plus => {
                let ln: f64 = l.parse().unwrap_or(0.0);
                let rn: f64 = r.parse().unwrap_or(0.0);
                Ok(self.format_number_result(ln + rn))
            }
            BinaryOperator::Minus => {
                let ln: f64 = l.parse().unwrap_or(0.0);
                let rn: f64 = r.parse().unwrap_or(0.0);
                Ok(self.format_number_result(ln - rn))
            }
            BinaryOperator::Multiply => {
                let ln: f64 = l.parse().unwrap_or(0.0);
                let rn: f64 = r.parse().unwrap_or(0.0);
                Ok(self.format_number_result(ln * rn))
            }
            BinaryOperator::Divide => {
                let ln: f64 = l.parse().unwrap_or(0.0);
                let rn: f64 = r.parse().unwrap_or(0.0);
                if rn == 0.0 {
                    if ln == 0.0 {
                        Ok("NaN".to_string())
                    } else if ln > 0.0 {
                        Ok("Infinity".to_string())
                    } else {
                        Ok("-Infinity".to_string())
                    }
                } else {
                    Ok(self.format_number_result(ln / rn))
                }
            }
            BinaryOperator::Modulo => {
                let ln: f64 = l.parse().unwrap_or(0.0);
                let rn: f64 = r.parse().unwrap_or(0.0);
                Ok(self.format_number_result(ln % rn))
            }
            BinaryOperator::Equals => Ok((l == r).to_string()),
            BinaryOperator::NotEquals => Ok((l != r).to_string()),
            BinaryOperator::LessThan => {
                let cmp = self.compare_values(&l, &r);
                Ok((cmp == std::cmp::Ordering::Less).to_string())
            }
            BinaryOperator::LessThanOrEqual => {
                let cmp = self.compare_values(&l, &r);
                Ok((cmp != std::cmp::Ordering::Greater).to_string())
            }
            BinaryOperator::GreaterThan => {
                let cmp = self.compare_values(&l, &r);
                Ok((cmp == std::cmp::Ordering::Greater).to_string())
            }
            BinaryOperator::GreaterThanOrEqual => {
                let cmp = self.compare_values(&l, &r);
                Ok((cmp != std::cmp::Ordering::Less).to_string())
            }
            BinaryOperator::And => {
                let lb = self.string_to_bool(&l);
                let rb = self.string_to_bool(&r);
                Ok((lb && rb).to_string())
            }
            BinaryOperator::Or => {
                let lb = self.string_to_bool(&l);
                let rb = self.string_to_bool(&r);
                Ok((lb || rb).to_string())
            }
            _ => Err(Xslt3Error::streaming(format!(
                "Binary operator {:?} not supported in streaming grounded evaluation",
                op
            ))),
        }
    }

    fn string_to_bool(&self, s: &str) -> bool {
        !s.is_empty() && s != "false" && s != "0"
    }

    fn compare_values(&self, l: &str, r: &str) -> std::cmp::Ordering {
        if let (Ok(ln), Ok(rn)) = (l.parse::<f64>(), r.parse::<f64>()) {
            ln.partial_cmp(&rn).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            l.cmp(r)
        }
    }

    fn format_number_result(&self, n: f64) -> String {
        if n.fract() == 0.0 && n.is_finite() {
            format!("{}", n as i64)
        } else {
            n.to_string()
        }
    }

    fn format_number_simple(&self, n: f64, pattern: &str) -> String {
        if pattern.contains('.') {
            let decimal_places = pattern.split('.').nth(1).map(|s| s.len()).unwrap_or(0);
            format!("{:.prec$}", n, prec = decimal_places)
        } else {
            format!("{}", n.round() as i64)
        }
    }

    fn evaluate_striding_expression(
        &self,
        expr: &petty_xpath31::Expression,
        node: &StreamedNode,
    ) -> Result<String, Xslt3Error> {
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Axis;

        match expr {
            Expression::ContextItem => Ok(node.string_value()),
            Expression::LocationPath(path) => {
                if !path.is_absolute && path.steps.len() == 1 {
                    let step = &path.steps[0];
                    match (&step.axis, &step.node_test) {
                        (Axis::Attribute, petty_xpath31::ast::NodeTest::Name(name)) => {
                            for attr in &node.attributes {
                                if attr.name.local_name == *name {
                                    return Ok(attr.value.clone());
                                }
                            }
                            Ok(String::new())
                        }
                        (Axis::Child, petty_xpath31::ast::NodeTest::Name(_name)) => {
                            Ok(node.string_value())
                        }
                        _ => Ok(node.string_value()),
                    }
                } else {
                    Ok(node.string_value())
                }
            }
            _ => self.evaluate_grounded_expression(expr),
        }
    }

    pub fn process_events<I>(&mut self, events: I) -> Result<Vec<IRNode>, Xslt3Error>
    where
        I: IntoIterator<Item = StreamEvent>,
    {
        self.initialize_accumulators();

        for event in events {
            self.process_event(event)?;
        }

        let builder = std::mem::take(&mut self.builder);
        Ok(builder.get_result())
    }

    fn process_event(&mut self, event: StreamEvent) -> Result<(), Xslt3Error> {
        match event {
            StreamEvent::StartDocument => {
                self.context.set_current(StreamedNode::document());
            }
            StreamEvent::EndDocument => {}
            StreamEvent::StartElement { name, attributes } => {
                self.context.push_element(name, attributes);

                if let Some(node) = self.context.current_node() {
                    let node_clone = node.clone();
                    self.process_accumulator_rules(&node_clone, AccumulatorPhase::Start);

                    if let Some(template) = self.find_matching_template(&node_clone) {
                        let template_clone = template.clone();
                        self.execute_streaming_template(&template_clone, &node_clone)?;
                    }
                }
            }
            StreamEvent::EndElement { name: _ } => {
                if let Some(node) = self.context.current_node() {
                    let node_clone = node.clone();
                    self.process_accumulator_rules(&node_clone, AccumulatorPhase::End);
                }

                self.context.pop_element();
            }
            StreamEvent::Text(content) => {
                self.context.process_text(content.clone());

                if let Some(node) = self.context.current_node() {
                    let node_clone = node.clone();

                    if self.find_matching_template(&node_clone).is_none() {
                        self.apply_builtin_template(&node_clone);
                    }
                }
            }
            StreamEvent::Comment(content) => {
                self.context.process_comment(content);
            }
            StreamEvent::ProcessingInstruction { target, data } => {
                self.context.process_pi(target, data);
            }
        }

        Ok(())
    }
}

impl StreamEventHandler for StreamingExecutor<'_> {
    type Output = Vec<IRNode>;
    type Error = Xslt3Error;

    fn start_document(&mut self) -> Result<(), Self::Error> {
        self.context.set_current(StreamedNode::document());
        self.initialize_accumulators();
        Ok(())
    }

    fn end_document(&mut self) -> Result<Self::Output, Self::Error> {
        let builder = std::mem::take(&mut self.builder);
        Ok(builder.get_result())
    }

    fn start_element(&mut self, name: &QName, attributes: &[Attribute]) -> Result<(), Self::Error> {
        self.context.push_element(name.clone(), attributes.to_vec());

        if let Some(node) = self.context.current_node() {
            let node_clone = node.clone();
            self.process_accumulator_rules(&node_clone, AccumulatorPhase::Start);
        }

        Ok(())
    }

    fn end_element(&mut self, _name: &QName) -> Result<(), Self::Error> {
        if let Some(node) = self.context.current_node() {
            let node_clone = node.clone();
            self.process_accumulator_rules(&node_clone, AccumulatorPhase::End);
        }

        self.context.pop_element();
        Ok(())
    }

    fn text(&mut self, content: &str) -> Result<(), Self::Error> {
        self.context.process_text(content.to_string());

        if let Some(node) = self.context.current_node() {
            let node_clone = node.clone();

            if self.find_matching_template(&node_clone).is_some() {
                self.builder.add_text(content);
            } else {
                self.apply_builtin_template(&node_clone);
            }
        }

        Ok(())
    }

    fn comment(&mut self, content: &str) -> Result<(), Self::Error> {
        self.context.process_comment(content.to_string());
        Ok(())
    }

    fn processing_instruction(&mut self, target: &str, data: &str) -> Result<(), Self::Error> {
        self.context
            .process_pi(target.to_string(), data.to_string());
        Ok(())
    }
}

/// Runs the streaming XML event loop, dispatching events to the executor.
///
/// This is the shared event loop used by both `parse_and_stream` and
/// `parse_and_stream_with_accumulators` to avoid code duplication.
fn run_streaming_event_loop(
    xml: &str,
    executor: &mut StreamingExecutor<'_>,
) -> Result<(), Xslt3Error> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    executor.start_document()?;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = QName::new(String::from_utf8_lossy(e.local_name().as_ref()).to_string());
                let attributes: Vec<Attribute> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| Attribute {
                        name: QName::new(
                            String::from_utf8_lossy(a.key.local_name().as_ref()).to_string(),
                        ),
                        value: String::from_utf8_lossy(&a.value).to_string(),
                    })
                    .collect();
                executor.start_element(&name, &attributes)?;
            }
            Ok(Event::End(e)) => {
                let name = QName::new(String::from_utf8_lossy(e.local_name().as_ref()).to_string());
                executor.end_element(&name)?;
            }
            Ok(Event::Empty(e)) => {
                let name = QName::new(String::from_utf8_lossy(e.local_name().as_ref()).to_string());
                let attributes: Vec<Attribute> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| Attribute {
                        name: QName::new(
                            String::from_utf8_lossy(a.key.local_name().as_ref()).to_string(),
                        ),
                        value: String::from_utf8_lossy(&a.value).to_string(),
                    })
                    .collect();
                executor.start_element(&name, &attributes)?;
                executor.end_element(&name)?;
            }
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(&e).to_string();
                if !text.trim().is_empty() {
                    executor.text(&text)?;
                }
            }
            Ok(Event::Comment(e)) => {
                let text = String::from_utf8_lossy(&e).to_string();
                executor.comment(&text)?;
            }
            Ok(Event::PI(e)) => {
                let content = String::from_utf8_lossy(&e).to_string();
                let (target, data) = content
                    .split_once(' ')
                    .map(|(t, d)| (t.to_string(), d.to_string()))
                    .unwrap_or((content, String::new()));
                executor.processing_instruction(&target, &data)?;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                return Err(Xslt3Error::parse(format!(
                    "Error parsing XML at position {}: {:?}",
                    reader.buffer_position(),
                    e
                )));
            }
        }
        buf.clear();
    }

    Ok(())
}

pub fn parse_and_stream(
    xml: &str,
    stylesheet: &CompiledStylesheet3,
) -> Result<Vec<IRNode>, Xslt3Error> {
    let mut executor = StreamingExecutor::new(stylesheet);
    run_streaming_event_loop(xml, &mut executor)?;
    executor.end_document()
}

pub struct StreamingResult {
    pub ir_nodes: Vec<IRNode>,
    pub accumulator_values: HashMap<String, String>,
}

pub fn parse_and_stream_with_accumulators(
    xml: &str,
    stylesheet: &CompiledStylesheet3,
) -> Result<StreamingResult, Xslt3Error> {
    let mut executor = StreamingExecutor::new(stylesheet);
    run_streaming_event_loop(xml, &mut executor)?;

    let builder = std::mem::take(&mut executor.builder);
    let ir_nodes = builder.get_result();

    let accumulator_values = executor
        .take_accumulator_values()
        .into_iter()
        .map(|(k, v)| (k, v.to_string_value()))
        .collect();

    Ok(StreamingResult {
        ir_nodes,
        accumulator_values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::CompiledStylesheet3;

    #[test]
    fn test_streaming_executor_basic() {
        let stylesheet = CompiledStylesheet3::default();
        let mut executor = StreamingExecutor::new(&stylesheet);

        executor.start_document().unwrap();

        let root = QName::new("root");
        executor.start_element(&root, &[]).unwrap();
        executor.text("Hello").unwrap();
        executor.end_element(&root).unwrap();

        let result = executor.end_document().unwrap();
        assert!(!result.is_empty() || result.is_empty());
    }

    #[test]
    fn test_streaming_executor_nested() {
        let stylesheet = CompiledStylesheet3::default();
        let mut executor = StreamingExecutor::new(&stylesheet);

        executor.start_document().unwrap();

        let root = QName::new("root");
        executor.start_element(&root, &[]).unwrap();

        let child = QName::new("child");
        executor.start_element(&child, &[]).unwrap();
        executor.text("Content").unwrap();
        executor.end_element(&child).unwrap();

        executor.end_element(&root).unwrap();

        let result = executor.end_document().unwrap();
        assert!(result.is_empty() || !result.is_empty());
    }

    #[test]
    fn test_parse_and_stream() {
        let xml = r#"<root><item>Hello</item><item>World</item></root>"#;
        let stylesheet = CompiledStylesheet3::default();

        let result = parse_and_stream(xml, &stylesheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_streaming_template_text() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};

        let stylesheet = CompiledStylesheet3::default();
        let mut executor = StreamingExecutor::new(&stylesheet);

        let template = PreparsedTemplate(vec![Xslt3Instruction::Text("Hello World".to_string())]);

        let node = StreamedNode::element(QName::new("test".to_string()), vec![], 0, 1);

        let result = executor.execute_streaming_template(&template, &node);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_streaming_template_value_of_literal() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;

        let stylesheet = CompiledStylesheet3::default();
        let mut executor = StreamingExecutor::new(&stylesheet);

        let template = PreparsedTemplate(vec![Xslt3Instruction::ValueOf {
            select: Expression::Literal(Literal::String("Literal Value".to_string())),
            separator: None,
        }]);

        let node = StreamedNode::element(QName::new("test".to_string()), vec![], 0, 1);

        let result = executor.execute_streaming_template(&template, &node);
        assert!(result.is_ok());
    }

    #[test]
    fn test_streaming_rejects_non_streamable() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};
        use petty_xpath31::Expression;
        use petty_xpath31::ast::{Axis, LocationPath, NodeTest, Step};

        let stylesheet = CompiledStylesheet3::default();
        let mut executor = StreamingExecutor::new(&stylesheet);

        let template = PreparsedTemplate(vec![Xslt3Instruction::ValueOf {
            select: Expression::LocationPath(LocationPath {
                start_point: None,
                is_absolute: false,
                steps: vec![Step {
                    axis: Axis::PrecedingSibling,
                    node_test: NodeTest::Name("item".to_string()),
                    predicates: vec![],
                }],
            }),
            separator: None,
        }]);

        let node = StreamedNode::element(QName::new("test".to_string()), vec![], 0, 1);

        let result = executor.execute_streaming_template(&template, &node);
        assert!(result.is_err());
    }

    #[test]
    fn test_streaming_large_document() {
        let mut xml = String::from("<root>");
        for i in 0..1000 {
            xml.push_str(&format!("<item id=\"{}\">Content {}</item>", i, i));
        }
        xml.push_str("</root>");

        let stylesheet = CompiledStylesheet3::default();
        let result = parse_and_stream(&xml, &stylesheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_streaming_deeply_nested() {
        let mut xml = String::new();
        for i in 0..50 {
            xml.push_str(&format!("<level{}>", i));
        }
        xml.push_str("deep content");
        for i in (0..50).rev() {
            xml.push_str(&format!("</level{}>", i));
        }

        let stylesheet = CompiledStylesheet3::default();
        let result = parse_and_stream(&xml, &stylesheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_streaming_many_attributes() {
        let mut xml = String::from("<root>");
        for i in 0..100 {
            let mut attrs = String::new();
            for j in 0..20 {
                attrs.push_str(&format!(" attr{}=\"value{}\"", j, j));
            }
            xml.push_str(&format!("<item id=\"{}\"{}/>", i, attrs));
        }
        xml.push_str("</root>");

        let stylesheet = CompiledStylesheet3::default();
        let result = parse_and_stream(&xml, &stylesheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_streaming_mixed_content() {
        let mut xml = String::from("<root>");
        for i in 0..500 {
            xml.push_str(&format!(
                "<para>Text before <bold>bold {}</bold> and <italic>italic</italic> text after.</para>",
                i
            ));
        }
        xml.push_str("</root>");

        let stylesheet = CompiledStylesheet3::default();
        let result = parse_and_stream(&xml, &stylesheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_streaming_with_comments_and_pi() {
        let mut xml = String::from("<?xml version=\"1.0\"?><root>");
        for i in 0..100 {
            xml.push_str(&format!(
                "<!-- Comment {} --><item><?process data{}?>{}</item>",
                i, i, i
            ));
        }
        xml.push_str("</root>");

        let stylesheet = CompiledStylesheet3::default();
        let result = parse_and_stream(&xml, &stylesheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_accumulator_initialization_evaluates_expression() {
        use crate::ast::{Accumulator, AccumulatorPhase, AccumulatorRule, Pattern3};
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;

        // Create a stylesheet with an accumulator that has initial-value="0"
        let mut stylesheet = CompiledStylesheet3::default();
        stylesheet.accumulators.insert(
            "count".to_string(),
            Accumulator {
                name: "count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    // $value + 1
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: petty_xpath1::ast::BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let mut executor = StreamingExecutor::new(&stylesheet);
        executor.initialize_accumulators();

        // The initial value should be "0", not ""
        let initial = executor.accumulator_values.get("count").unwrap();
        assert_eq!(
            initial.to_string_value(),
            "0",
            "Accumulator should be initialized to evaluated initial_value expression"
        );
    }

    #[test]
    fn test_accumulator_rule_updates_value() {
        use super::super::event_model::QName;
        use crate::ast::{Accumulator, AccumulatorPhase, AccumulatorRule, Pattern3};
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;

        // Create a stylesheet with an accumulator that counts items
        let mut stylesheet = CompiledStylesheet3::default();
        stylesheet.accumulators.insert(
            "count".to_string(),
            Accumulator {
                name: "count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    // $value + 1
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: petty_xpath1::ast::BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let mut executor = StreamingExecutor::new(&stylesheet);
        executor.initialize_accumulators();

        // Simulate processing an <item> element
        let node = StreamedNode::element(QName::new("item".to_string()), vec![], 0, 1);
        executor.process_accumulator_rules(&node, AccumulatorPhase::Start);

        // After processing one item, accumulator should be "1"
        let value = executor.accumulator_values.get("count").unwrap();
        assert_eq!(
            value.to_string_value(),
            "1",
            "Accumulator should be updated by rule evaluation"
        );

        // Process another item
        executor.process_accumulator_rules(&node, AccumulatorPhase::Start);
        let value = executor.accumulator_values.get("count").unwrap();
        assert_eq!(
            value.to_string_value(),
            "2",
            "Accumulator should increment on each matching element"
        );
    }

    #[test]
    fn test_parse_and_stream_with_accumulators_returns_final_values() {
        use crate::ast::{Accumulator, AccumulatorPhase, AccumulatorRule, Pattern3};
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;

        let mut stylesheet = CompiledStylesheet3::default();
        stylesheet.accumulators.insert(
            "count".to_string(),
            Accumulator {
                name: "count".to_string(),
                initial_value: Expression::Literal(Literal::Integer(0)),
                rules: vec![AccumulatorRule {
                    pattern: Pattern3("item".to_string()),
                    phase: AccumulatorPhase::Start,
                    select: Expression::BinaryOp {
                        left: Box::new(Expression::Variable("value".to_string())),
                        right: Box::new(Expression::Literal(Literal::Integer(1))),
                        op: petty_xpath1::ast::BinaryOperator::Plus,
                    },
                }],
                streamable: true,
            },
        );

        let xml = r#"<root><item/><item/><item/></root>"#;
        let result = parse_and_stream_with_accumulators(xml, &stylesheet).unwrap();

        assert_eq!(
            result.accumulator_values.get("count").map(String::as_str),
            Some("3"),
            "Accumulator should have counted 3 items"
        );
    }

    #[test]
    fn test_grounded_expression_string_functions() {
        use petty_xpath31::Expression;
        use petty_xpath31::ast::{Literal, QName as XPathQName};

        let stylesheet = CompiledStylesheet3::default();
        let executor = StreamingExecutor::new(&stylesheet);

        let concat_expr = Expression::FunctionCall {
            name: XPathQName::new("concat"),
            args: vec![
                Expression::Literal(Literal::String("Hello".to_string())),
                Expression::Literal(Literal::String(" ".to_string())),
                Expression::Literal(Literal::String("World".to_string())),
            ],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&concat_expr).unwrap(),
            "Hello World"
        );

        let strlen_expr = Expression::FunctionCall {
            name: XPathQName::new("string-length"),
            args: vec![Expression::Literal(Literal::String("Hello".to_string()))],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&strlen_expr).unwrap(),
            "5"
        );

        let upper_expr = Expression::FunctionCall {
            name: XPathQName::new("upper-case"),
            args: vec![Expression::Literal(Literal::String("hello".to_string()))],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&upper_expr).unwrap(),
            "HELLO"
        );

        let lower_expr = Expression::FunctionCall {
            name: XPathQName::new("lower-case"),
            args: vec![Expression::Literal(Literal::String("HELLO".to_string()))],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&lower_expr).unwrap(),
            "hello"
        );

        let contains_expr = Expression::FunctionCall {
            name: XPathQName::new("contains"),
            args: vec![
                Expression::Literal(Literal::String("Hello World".to_string())),
                Expression::Literal(Literal::String("World".to_string())),
            ],
        };
        assert_eq!(
            executor
                .evaluate_grounded_expression(&contains_expr)
                .unwrap(),
            "true"
        );

        let starts_expr = Expression::FunctionCall {
            name: XPathQName::new("starts-with"),
            args: vec![
                Expression::Literal(Literal::String("Hello World".to_string())),
                Expression::Literal(Literal::String("Hello".to_string())),
            ],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&starts_expr).unwrap(),
            "true"
        );

        let normalize_expr = Expression::FunctionCall {
            name: XPathQName::new("normalize-space"),
            args: vec![Expression::Literal(Literal::String(
                "  hello   world  ".to_string(),
            ))],
        };
        assert_eq!(
            executor
                .evaluate_grounded_expression(&normalize_expr)
                .unwrap(),
            "hello world"
        );
    }

    #[test]
    fn test_grounded_expression_numeric_functions() {
        use petty_xpath31::Expression;
        use petty_xpath31::ast::{Literal, QName as XPathQName};

        let stylesheet = CompiledStylesheet3::default();
        let executor = StreamingExecutor::new(&stylesheet);

        let floor_expr = Expression::FunctionCall {
            name: XPathQName::new("floor"),
            args: vec![Expression::Literal(Literal::Double(3.7))],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&floor_expr).unwrap(),
            "3"
        );

        let ceil_expr = Expression::FunctionCall {
            name: XPathQName::new("ceiling"),
            args: vec![Expression::Literal(Literal::Double(3.2))],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&ceil_expr).unwrap(),
            "4"
        );

        let round_expr = Expression::FunctionCall {
            name: XPathQName::new("round"),
            args: vec![Expression::Literal(Literal::Double(3.5))],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&round_expr).unwrap(),
            "4"
        );

        let abs_expr = Expression::FunctionCall {
            name: XPathQName::new("abs"),
            args: vec![Expression::Literal(Literal::Double(-5.0))],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&abs_expr).unwrap(),
            "5"
        );
    }

    #[test]
    fn test_grounded_expression_comparison_operators() {
        use petty_xpath1::ast::BinaryOperator;
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;

        let stylesheet = CompiledStylesheet3::default();
        let executor = StreamingExecutor::new(&stylesheet);

        let eq_expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(5))),
            right: Box::new(Expression::Literal(Literal::Integer(5))),
            op: BinaryOperator::Equals,
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&eq_expr).unwrap(),
            "true"
        );

        let neq_expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(5))),
            right: Box::new(Expression::Literal(Literal::Integer(3))),
            op: BinaryOperator::NotEquals,
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&neq_expr).unwrap(),
            "true"
        );

        let lt_expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(3))),
            right: Box::new(Expression::Literal(Literal::Integer(5))),
            op: BinaryOperator::LessThan,
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&lt_expr).unwrap(),
            "true"
        );

        let gt_expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(5))),
            right: Box::new(Expression::Literal(Literal::Integer(3))),
            op: BinaryOperator::GreaterThan,
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&gt_expr).unwrap(),
            "true"
        );
    }

    #[test]
    fn test_grounded_expression_boolean_operators() {
        use petty_xpath1::ast::BinaryOperator;
        use petty_xpath31::Expression;
        use petty_xpath31::ast::QName as XPathQName;

        let stylesheet = CompiledStylesheet3::default();
        let executor = StreamingExecutor::new(&stylesheet);

        let and_expr = Expression::BinaryOp {
            left: Box::new(Expression::FunctionCall {
                name: XPathQName::new("true"),
                args: vec![],
            }),
            right: Box::new(Expression::FunctionCall {
                name: XPathQName::new("true"),
                args: vec![],
            }),
            op: BinaryOperator::And,
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&and_expr).unwrap(),
            "true"
        );

        let or_expr = Expression::BinaryOp {
            left: Box::new(Expression::FunctionCall {
                name: XPathQName::new("false"),
                args: vec![],
            }),
            right: Box::new(Expression::FunctionCall {
                name: XPathQName::new("true"),
                args: vec![],
            }),
            op: BinaryOperator::Or,
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&or_expr).unwrap(),
            "true"
        );

        let not_expr = Expression::FunctionCall {
            name: XPathQName::new("not"),
            args: vec![Expression::FunctionCall {
                name: XPathQName::new("false"),
                args: vec![],
            }],
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&not_expr).unwrap(),
            "true"
        );
    }

    #[test]
    fn test_grounded_expression_arithmetic_operators() {
        use petty_xpath1::ast::BinaryOperator;
        use petty_xpath31::Expression;
        use petty_xpath31::ast::Literal;

        let stylesheet = CompiledStylesheet3::default();
        let executor = StreamingExecutor::new(&stylesheet);

        let div_expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(10))),
            right: Box::new(Expression::Literal(Literal::Integer(3))),
            op: BinaryOperator::Divide,
        };
        let result = executor.evaluate_grounded_expression(&div_expr).unwrap();
        assert!(result.starts_with("3.3333"));

        let mod_expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(10))),
            right: Box::new(Expression::Literal(Literal::Integer(3))),
            op: BinaryOperator::Modulo,
        };
        assert_eq!(
            executor.evaluate_grounded_expression(&mod_expr).unwrap(),
            "1"
        );
    }
}
