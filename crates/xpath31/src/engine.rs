//! XPath 3.1 expression evaluation engine.
//!
//! Entry point: [`evaluate`] with an [`EvaluationContext`].

use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

use crate::ast::*;
use crate::error::XPath31Error;
use crate::functions;
use crate::operators;
use crate::types::*;
use petty_xpath1::DataSourceNode;

pub struct EvaluationContext<'a, 'd, N: DataSourceNode<'a>> {
    pub context_item: Option<XdmItem<N>>,
    pub context_position: usize,
    pub context_size: usize,
    pub variables: &'d HashMap<String, XdmValue<N>>,
    pub root_node: Option<N>,
    _marker: PhantomData<&'a ()>,
}

impl<'a, 'd, N: DataSourceNode<'a> + Clone + 'a> EvaluationContext<'a, 'd, N> {
    pub fn new(
        context_item: Option<XdmItem<N>>,
        root_node: Option<N>,
        variables: &'d HashMap<String, XdmValue<N>>,
    ) -> Self {
        Self {
            context_item,
            context_position: 1,
            context_size: 1,
            variables,
            root_node,
            _marker: PhantomData,
        }
    }

    pub fn with_context_item(&self, item: XdmItem<N>) -> Self {
        Self {
            context_item: Some(item),
            context_position: self.context_position,
            context_size: self.context_size,
            variables: self.variables,
            root_node: self.root_node,
            _marker: PhantomData,
        }
    }

    pub fn with_position(&self, position: usize, size: usize) -> Self {
        Self {
            context_item: self.context_item.clone(),
            context_position: position,
            context_size: size,
            variables: self.variables,
            root_node: self.root_node,
            _marker: PhantomData,
        }
    }
}

pub fn evaluate<'a, N>(
    expr: &Expression,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error>
where
    N: DataSourceNode<'a> + Clone + 'a,
{
    match expr {
        Expression::Literal(lit) => evaluate_literal(lit),
        Expression::Variable(name) => evaluate_variable(name, ctx, local_vars),
        Expression::ContextItem => evaluate_context_item(ctx),

        Expression::LetExpr {
            bindings,
            return_expr,
        } => evaluate_let(bindings, return_expr, ctx, local_vars),
        Expression::IfExpr {
            condition,
            then_expr,
            else_expr,
        } => evaluate_if(condition, then_expr, else_expr, ctx, local_vars),
        Expression::ForExpr {
            bindings,
            return_expr,
        } => evaluate_for(bindings, return_expr, ctx, local_vars),
        Expression::QuantifiedExpr {
            quantifier,
            bindings,
            satisfies,
        } => evaluate_quantified(*quantifier, bindings, satisfies, ctx, local_vars),

        Expression::BinaryOp { left, op, right } => {
            let l = evaluate(left, ctx, local_vars)?;
            let r = evaluate(right, ctx, local_vars)?;
            operators::evaluate_binary_with_nodes(*op, l, r)
        }
        Expression::UnaryOp { op, expr } => {
            let val = evaluate(expr, ctx, local_vars)?;
            match op {
                petty_xpath1::ast::UnaryOperator::Minus => {
                    Ok(XdmValue::from_double(-val.to_double()))
                }
                petty_xpath1::ast::UnaryOperator::Plus => {
                    Ok(XdmValue::from_double(val.to_double()))
                }
            }
        }

        Expression::StringConcat { left, right } => {
            let l = evaluate(left, ctx, local_vars)?;
            let r = evaluate(right, ctx, local_vars)?;
            Ok(XdmValue::from_string(format!(
                "{}{}",
                l.to_string_value(),
                r.to_string_value()
            )))
        }

        Expression::RangeExpr { start, end } => evaluate_range(start, end, ctx, local_vars),

        Expression::MapConstructor(entries) => evaluate_map_constructor(entries, ctx, local_vars),
        Expression::ArrayConstructor(kind) => evaluate_array_constructor(kind, ctx, local_vars),

        Expression::ArrowExpr { base, steps } => evaluate_arrow(base, steps, ctx, local_vars),
        Expression::SimpleMapExpr { base, mapping } => {
            evaluate_simple_map(base, mapping, ctx, local_vars)
        }
        Expression::LookupExpr { base, key } => evaluate_lookup(base, key, ctx, local_vars),
        Expression::UnaryLookup(key) => evaluate_unary_lookup(key, ctx, local_vars),

        Expression::FunctionCall { name, args } => {
            evaluate_function_call(name, args, ctx, local_vars)
        }
        Expression::InlineFunction { params, body, .. } => {
            evaluate_inline_function(params, body, local_vars)
        }
        Expression::NamedFunctionRef { name, arity } => Ok(XdmValue::from_function(
            XdmFunction::named_ref(name.prefix.clone(), name.local_part.clone(), *arity),
        )),

        Expression::Sequence(exprs) => {
            let mut items = Vec::new();
            for e in exprs {
                let val = evaluate(e, ctx, local_vars)?;
                items.extend(val.into_items());
            }
            Ok(XdmValue::from_items(items))
        }

        Expression::LocationPath(path) => evaluate_location_path(path, ctx),

        Expression::FilterExpr { base, predicates } => {
            evaluate_filter_expr(base, predicates, ctx, local_vars)
        }

        Expression::DynamicFunctionCall {
            function_expr,
            args,
        } => evaluate_dynamic_function_call(function_expr, args, ctx, local_vars),

        Expression::InstanceOf {
            expr,
            sequence_type,
        } => evaluate_instance_of(expr, sequence_type, ctx, local_vars),

        Expression::TreatAs {
            expr,
            sequence_type,
        } => evaluate_treat_as(expr, sequence_type, ctx, local_vars),

        Expression::CastAs { expr, single_type } => {
            evaluate_cast_as(expr, single_type, ctx, local_vars)
        }

        Expression::CastableAs { expr, single_type } => {
            evaluate_castable_as(expr, single_type, ctx, local_vars)
        }

        Expression::ArgumentPlaceholder => Err(XPath31Error::type_error(
            "Argument placeholder '?' can only be used in function calls",
        )),
    }
}

fn evaluate_literal<N: Clone>(lit: &Literal) -> Result<XdmValue<N>, XPath31Error> {
    match lit {
        Literal::String(s) => Ok(XdmValue::from_string(s.clone())),
        Literal::Integer(i) => Ok(XdmValue::from_integer(*i)),
        Literal::Double(d) => Ok(XdmValue::from_double(*d)),
        Literal::Decimal(s) => {
            let d: f64 = s.parse().unwrap_or(0.0);
            Ok(XdmValue::from_double(d))
        }
    }
}

fn evaluate_variable<'a, N: DataSourceNode<'a> + Clone + 'a>(
    name: &str,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if let Some(val) = local_vars.get(name) {
        return Ok(val.clone());
    }
    if let Some(val) = ctx.variables.get(name) {
        return Ok(val.clone());
    }
    Err(XPath31Error::UnknownVariable {
        name: name.to_string(),
    })
}

fn evaluate_context_item<'a, N: DataSourceNode<'a> + Clone + 'a>(
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    match &ctx.context_item {
        Some(item) => Ok(XdmValue::from_item(item.clone())),
        None => Err(XPath31Error::NoContextItem),
    }
}

fn evaluate_location_path<'a, N: DataSourceNode<'a> + Clone + 'a>(
    path: &LocationPath,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    let initial_nodes: Vec<N> = if path.is_absolute {
        match ctx.root_node {
            Some(root) => vec![root],
            None => return Ok(XdmValue::empty()),
        }
    } else {
        match &ctx.context_item {
            Some(XdmItem::Node(node)) => vec![*node],
            _ => return Ok(XdmValue::empty()),
        }
    };

    if path.steps.is_empty() {
        let items: Vec<XdmItem<N>> = initial_nodes.into_iter().map(XdmItem::Node).collect();
        return Ok(XdmValue::from_items(items));
    }

    let mut current_nodes = initial_nodes;
    for step in &path.steps {
        current_nodes = evaluate_step(step, &current_nodes)?;
    }

    let items: Vec<XdmItem<N>> = current_nodes.into_iter().map(XdmItem::Node).collect();
    Ok(XdmValue::from_items(items))
}

fn evaluate_step<'a, N: DataSourceNode<'a> + Clone + 'a>(
    step: &Step,
    context_nodes: &[N],
) -> Result<Vec<N>, XPath31Error> {
    let mut result: Vec<N> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    for &node in context_nodes {
        let axis_nodes: Vec<N> = match step.axis {
            Axis::Child => node.children().collect(),
            Axis::Parent => node.parent().into_iter().collect(),
            Axis::SelfAxis => vec![node],
            Axis::Descendant => collect_descendants(node),
            Axis::DescendantOrSelf => {
                let mut nodes = vec![node];
                nodes.extend(collect_descendants(node));
                nodes
            }
            Axis::Ancestor => collect_ancestors(node),
            Axis::Attribute => node.attributes().collect(),
            Axis::FollowingSibling => collect_following_siblings(node),
            Axis::PrecedingSibling => collect_preceding_siblings(node),
            Axis::Following => collect_following(node),
            Axis::Preceding => collect_preceding(node),
        };

        for n in axis_nodes {
            if matches_node_test(&n, &step.node_test, step.axis) {
                let key = compute_node_hash(&n);
                if seen.insert(key) {
                    result.push(n);
                }
            }
        }
    }

    Ok(result)
}

fn compute_node_hash<'a, N: DataSourceNode<'a>>(node: &N) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.hash(&mut hasher);
    hasher.finish()
}

fn matches_node_test<'a, N: DataSourceNode<'a>>(node: &N, test: &NodeTest, axis: Axis) -> bool {
    use petty_xpath1::datasource::NodeType;

    match test {
        NodeTest::Wildcard => match axis {
            Axis::Attribute => node.node_type() == NodeType::Attribute,
            _ => node.node_type() == NodeType::Element,
        },
        NodeTest::Name(name) => {
            if let Some(qname) = node.name() {
                qname.local_part == name || name == "*"
            } else {
                false
            }
        }
        NodeTest::NodeType(kind) => match kind {
            NodeTypeTest::Node => true,
            NodeTypeTest::Text => node.node_type() == NodeType::Text,
            NodeTypeTest::Comment => node.node_type() == NodeType::Comment,
            NodeTypeTest::ProcessingInstruction => {
                node.node_type() == NodeType::ProcessingInstruction
            }
        },
    }
}

fn collect_descendants<'a, N: DataSourceNode<'a>>(node: N) -> Vec<N> {
    let mut result: Vec<N> = Vec::new();
    for child in node.children() {
        result.push(child);
        result.extend(collect_descendants(child));
    }
    result
}

fn collect_ancestors<'a, N: DataSourceNode<'a>>(node: N) -> Vec<N> {
    let mut result: Vec<N> = Vec::new();
    let mut current = node.parent();
    while let Some(parent) = current {
        result.push(parent);
        current = parent.parent();
    }
    result
}

fn collect_following_siblings<'a, N: DataSourceNode<'a>>(node: N) -> Vec<N> {
    if let Some(parent) = node.parent() {
        let siblings: Vec<N> = parent.children().collect();
        let mut found_self = false;
        siblings
            .into_iter()
            .filter(|n| {
                if *n == node {
                    found_self = true;
                    false
                } else {
                    found_self
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

fn collect_preceding_siblings<'a, N: DataSourceNode<'a>>(node: N) -> Vec<N> {
    if let Some(parent) = node.parent() {
        let siblings: Vec<N> = parent.children().collect();
        siblings.into_iter().take_while(|n| *n != node).collect()
    } else {
        Vec::new()
    }
}

fn collect_following<'a, N: DataSourceNode<'a>>(node: N) -> Vec<N> {
    let mut result: Vec<N> = Vec::new();
    for sibling in collect_following_siblings(node) {
        result.push(sibling);
        result.extend(collect_descendants(sibling));
    }
    if let Some(parent) = node.parent() {
        result.extend(collect_following(parent));
    }
    result
}

fn collect_preceding<'a, N: DataSourceNode<'a>>(node: N) -> Vec<N> {
    let mut result: Vec<N> = Vec::new();
    for sibling in collect_preceding_siblings(node) {
        result.extend(collect_descendants(sibling));
        result.push(sibling);
    }
    if let Some(parent) = node.parent() {
        result.extend(collect_preceding(parent));
    }
    result
}

fn evaluate_let<'a, N: DataSourceNode<'a> + Clone + 'a>(
    bindings: &[(String, Box<Expression>)],
    return_expr: &Expression,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let mut new_vars = local_vars.clone();

    for (name, expr) in bindings {
        let val = evaluate(expr, ctx, &new_vars)?;
        new_vars.insert(name.clone(), val);
    }

    evaluate(return_expr, ctx, &new_vars)
}

fn evaluate_if<'a, N: DataSourceNode<'a> + Clone + 'a>(
    condition: &Expression,
    then_expr: &Expression,
    else_expr: &Expression,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let cond_val = evaluate(condition, ctx, local_vars)?;
    if cond_val.effective_boolean_value() {
        evaluate(then_expr, ctx, local_vars)
    } else {
        evaluate(else_expr, ctx, local_vars)
    }
}

fn evaluate_for<'a, N: DataSourceNode<'a> + Clone + 'a>(
    bindings: &[(String, Box<Expression>)],
    return_expr: &Expression,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    fn iterate<'a, N: DataSourceNode<'a> + Clone + 'a>(
        bindings: &[(String, Box<Expression>)],
        binding_idx: usize,
        return_expr: &Expression,
        ctx: &EvaluationContext<'a, '_, N>,
        local_vars: &HashMap<String, XdmValue<N>>,
    ) -> Result<Vec<XdmItem<N>>, XPath31Error> {
        if binding_idx >= bindings.len() {
            let result = evaluate(return_expr, ctx, local_vars)?;
            return Ok(result.into_items());
        }

        let (name, expr) = &bindings[binding_idx];
        let sequence = evaluate(expr, ctx, local_vars)?;
        let items = sequence.items();

        let mut results = Vec::new();
        for item in items {
            let mut new_vars = local_vars.clone();
            new_vars.insert(name.clone(), XdmValue::from_item(item.clone()));
            let sub_results = iterate(bindings, binding_idx + 1, return_expr, ctx, &new_vars)?;
            results.extend(sub_results);
        }

        Ok(results)
    }

    let items = iterate(bindings, 0, return_expr, ctx, local_vars)?;
    Ok(XdmValue::from_items(items))
}

fn evaluate_quantified<'a, N: DataSourceNode<'a> + Clone + 'a>(
    quantifier: Quantifier,
    bindings: &[(String, Box<Expression>)],
    satisfies: &Expression,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    fn check_quantified<'a, N: DataSourceNode<'a> + Clone + 'a>(
        quantifier: Quantifier,
        bindings: &[(String, Box<Expression>)],
        binding_idx: usize,
        satisfies: &Expression,
        ctx: &EvaluationContext<'a, '_, N>,
        local_vars: &HashMap<String, XdmValue<N>>,
    ) -> Result<bool, XPath31Error> {
        if binding_idx >= bindings.len() {
            let result = evaluate(satisfies, ctx, local_vars)?;
            return Ok(result.effective_boolean_value());
        }

        let (name, expr) = &bindings[binding_idx];
        let sequence = evaluate(expr, ctx, local_vars)?;
        let items = sequence.items();

        for item in items {
            let mut new_vars = local_vars.clone();
            new_vars.insert(name.clone(), XdmValue::from_item(item.clone()));
            let result = check_quantified(
                quantifier,
                bindings,
                binding_idx + 1,
                satisfies,
                ctx,
                &new_vars,
            )?;

            match quantifier {
                Quantifier::Some if result => return Ok(true),
                Quantifier::Every if !result => return Ok(false),
                _ => {}
            }
        }

        match quantifier {
            Quantifier::Some => Ok(false),
            Quantifier::Every => Ok(true),
        }
    }

    let result = check_quantified(quantifier, bindings, 0, satisfies, ctx, local_vars)?;
    Ok(XdmValue::from_bool(result))
}

fn evaluate_range<'a, N: DataSourceNode<'a> + Clone + 'a>(
    start: &Expression,
    end: &Expression,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let start_val = evaluate(start, ctx, local_vars)?;
    let end_val = evaluate(end, ctx, local_vars)?;

    let s = start_val.to_double() as i64;
    let e = end_val.to_double() as i64;

    if s > e {
        return Ok(XdmValue::empty());
    }

    let items: Vec<XdmItem<N>> = (s..=e)
        .map(|i| XdmItem::Atomic(AtomicValue::Integer(i)))
        .collect();

    Ok(XdmValue::from_items(items))
}

fn evaluate_map_constructor<'a, N: DataSourceNode<'a> + Clone + 'a>(
    entries: &[MapEntry],
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let mut map = XdmMap::new();

    for entry in entries {
        let key_val = evaluate(&entry.key, ctx, local_vars)?;
        let value_val = evaluate(&entry.value, ctx, local_vars)?;

        let key = match key_val.first() {
            Some(XdmItem::Atomic(a)) => a.clone(),
            _ => return Err(XPath31Error::type_error("Map key must be an atomic value")),
        };

        map = map.put(key, value_val);
    }

    Ok(XdmValue::from_map(map))
}

fn evaluate_array_constructor<'a, N: DataSourceNode<'a> + Clone + 'a>(
    kind: &ArrayConstructorKind,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    match kind {
        ArrayConstructorKind::Square(members) => {
            let mut arr_members = Vec::with_capacity(members.len());
            for m in members {
                let val = evaluate(m, ctx, local_vars)?;
                arr_members.push(val);
            }
            Ok(XdmValue::from_array(XdmArray::from_members(arr_members)))
        }
        ArrayConstructorKind::Curly(expr) => {
            let val = evaluate(expr, ctx, local_vars)?;
            let members: Vec<XdmValue<N>> = val
                .items()
                .iter()
                .map(|item| XdmValue::from_item(item.clone()))
                .collect();
            Ok(XdmValue::from_array(XdmArray::from_members(members)))
        }
    }
}

fn evaluate_arrow<'a, N: DataSourceNode<'a> + Clone + 'a>(
    base: &Expression,
    steps: &[ArrowStep],
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let mut current = evaluate(base, ctx, local_vars)?;

    for step in steps {
        let mut args = vec![current];
        for arg_expr in &step.args {
            args.push(evaluate(arg_expr, ctx, local_vars)?);
        }
        current = functions::call_function(&step.function_name, args, ctx, local_vars)?;
    }

    Ok(current)
}

fn evaluate_simple_map<'a, N: DataSourceNode<'a> + Clone + 'a>(
    base: &Expression,
    mapping: &Expression,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let base_val = evaluate(base, ctx, local_vars)?;
    let items = base_val.items();
    let size = items.len();

    let mut results = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let new_ctx = ctx
            .with_context_item(item.clone())
            .with_position(i + 1, size);
        let mapped = evaluate(mapping, &new_ctx, local_vars)?;
        results.extend(mapped.into_items());
    }

    Ok(XdmValue::from_items(results))
}

fn evaluate_filter_expr<'a, N: DataSourceNode<'a> + Clone + 'a>(
    base: &Expression,
    predicates: &[Expression],
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let base_val = evaluate(base, ctx, local_vars)?;
    let mut items = base_val.items().to_vec();

    for pred in predicates {
        let size = items.len();
        let mut filtered = Vec::new();

        for (i, item) in items.iter().enumerate() {
            let pred_ctx = ctx
                .with_context_item(item.clone())
                .with_position(i + 1, size);

            let pred_result = evaluate(pred, &pred_ctx, local_vars)?;

            let include = if let Some(XdmItem::Atomic(AtomicValue::Integer(pos))) =
                pred_result.first()
            {
                *pos as usize == i + 1
            } else if let Some(XdmItem::Atomic(AtomicValue::Double(pos))) = pred_result.first() {
                (*pos as usize) == i + 1
            } else {
                pred_result.effective_boolean_value()
            };

            if include {
                filtered.push(item.clone());
            }
        }

        items = filtered;
    }

    Ok(XdmValue::from_items(items))
}

fn evaluate_dynamic_function_call<'a, N: DataSourceNode<'a> + Clone + 'a>(
    function_expr: &Expression,
    args: &[Expression],
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let func_val = evaluate(function_expr, ctx, local_vars)?;

    let mut evaluated_args = Vec::with_capacity(args.len());
    for arg in args {
        evaluated_args.push(evaluate(arg, ctx, local_vars)?);
    }

    match func_val.first() {
        Some(XdmItem::Map(map)) => {
            if evaluated_args.len() != 1 {
                return Err(XPath31Error::type_error(
                    "Map lookup requires exactly one argument",
                ));
            }
            match evaluated_args[0].first() {
                Some(XdmItem::Atomic(key)) => match map.get(key) {
                    Some(val) => Ok(val.clone()),
                    None => Ok(XdmValue::empty()),
                },
                _ => Err(XPath31Error::type_error("Map key must be atomic")),
            }
        }
        Some(XdmItem::Array(arr)) => {
            if evaluated_args.len() != 1 {
                return Err(XPath31Error::type_error(
                    "Array lookup requires exactly one argument",
                ));
            }
            match evaluated_args[0].first() {
                Some(XdmItem::Atomic(AtomicValue::Integer(i))) => match arr.get(*i as usize) {
                    Some(val) => Ok(val.clone()),
                    None => Err(XPath31Error::ArrayIndexOutOfBounds {
                        index: *i,
                        size: arr.size(),
                    }),
                },
                _ => Err(XPath31Error::type_error("Array index must be an integer")),
            }
        }
        Some(XdmItem::Function(func)) => {
            functions::call_xdm_function(func, evaluated_args, ctx, local_vars)
        }
        _ => Err(XPath31Error::type_error(
            "Dynamic function call requires a function, map, or array",
        )),
    }
}

fn evaluate_lookup<'a, N: DataSourceNode<'a> + Clone + 'a>(
    base: &Expression,
    key: &LookupKey,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let base_val = evaluate(base, ctx, local_vars)?;

    match base_val.first() {
        Some(XdmItem::Map(map)) => lookup_in_map(map, key, ctx, local_vars),
        Some(XdmItem::Array(arr)) => lookup_in_array(arr, key, ctx, local_vars),
        _ => Err(XPath31Error::type_error("Lookup requires a map or array")),
    }
}

fn lookup_in_map<'a, N: DataSourceNode<'a> + Clone + 'a>(
    map: &XdmMap<N>,
    key: &LookupKey,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    match key {
        LookupKey::NCName(name) => {
            let atomic_key = AtomicValue::String(name.clone());
            match map.get(&atomic_key) {
                Some(val) => Ok(val.clone()),
                None => Ok(XdmValue::empty()),
            }
        }
        LookupKey::Integer(i) => {
            let atomic_key = AtomicValue::Integer(*i);
            match map.get(&atomic_key) {
                Some(val) => Ok(val.clone()),
                None => Ok(XdmValue::empty()),
            }
        }
        LookupKey::Parenthesized(expr) => {
            let key_val = evaluate(expr, ctx, local_vars)?;
            match key_val.first() {
                Some(XdmItem::Atomic(a)) => match map.get(a) {
                    Some(val) => Ok(val.clone()),
                    None => Ok(XdmValue::empty()),
                },
                _ => Err(XPath31Error::type_error("Map key must be atomic")),
            }
        }
        LookupKey::Wildcard => {
            let mut items = Vec::new();
            for val in map.values() {
                items.extend(val.clone().into_items());
            }
            Ok(XdmValue::from_items(items))
        }
    }
}

fn lookup_in_array<'a, N: DataSourceNode<'a> + Clone + 'a>(
    arr: &XdmArray<N>,
    key: &LookupKey,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    match key {
        LookupKey::Integer(i) => match arr.get(*i as usize) {
            Some(val) => Ok(val.clone()),
            None => Err(XPath31Error::ArrayIndexOutOfBounds {
                index: *i,
                size: arr.size(),
            }),
        },
        LookupKey::Parenthesized(expr) => {
            let key_val = evaluate(expr, ctx, local_vars)?;
            match key_val.first() {
                Some(XdmItem::Atomic(AtomicValue::Integer(i))) => match arr.get(*i as usize) {
                    Some(val) => Ok(val.clone()),
                    None => Err(XPath31Error::ArrayIndexOutOfBounds {
                        index: *i,
                        size: arr.size(),
                    }),
                },
                _ => Err(XPath31Error::type_error("Array index must be an integer")),
            }
        }
        LookupKey::Wildcard => {
            let mut items = Vec::new();
            for member in arr.iter() {
                items.extend(member.clone().into_items());
            }
            Ok(XdmValue::from_items(items))
        }
        LookupKey::NCName(_) => Err(XPath31Error::type_error(
            "Cannot use NCName to index into array",
        )),
    }
}

fn evaluate_unary_lookup<'a, N: DataSourceNode<'a> + Clone + 'a>(
    key: &LookupKey,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    match &ctx.context_item {
        Some(XdmItem::Map(map)) => lookup_in_map(map, key, ctx, local_vars),
        Some(XdmItem::Array(arr)) => lookup_in_array(arr, key, ctx, local_vars),
        Some(_) => Err(XPath31Error::type_error(
            "Context item must be a map or array for unary lookup",
        )),
        None => Err(XPath31Error::NoContextItem),
    }
}

fn evaluate_function_call<'a, N: DataSourceNode<'a> + Clone + 'a>(
    name: &QName,
    args: &[Expression],
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let has_placeholder = args
        .iter()
        .any(|a| matches!(a, Expression::ArgumentPlaceholder));

    if has_placeholder {
        let mut bound_args: Vec<Option<XdmValue<N>>> = Vec::with_capacity(args.len());
        for arg in args {
            if matches!(arg, Expression::ArgumentPlaceholder) {
                bound_args.push(None);
            } else {
                bound_args.push(Some(evaluate(arg, ctx, local_vars)?));
            }
        }

        let base_func =
            XdmFunction::named_ref(name.prefix.clone(), name.local_part.clone(), args.len());

        Ok(XdmValue::from_function(XdmFunction::Partial {
            base: Box::new(base_func),
            bound_args,
        }))
    } else {
        let mut evaluated_args = Vec::with_capacity(args.len());
        for arg in args {
            evaluated_args.push(evaluate(arg, ctx, local_vars)?);
        }
        functions::call_function(name, evaluated_args, ctx, local_vars)
    }
}

fn evaluate_inline_function<'a, N: DataSourceNode<'a> + Clone + 'a>(
    params: &[Param],
    body: &Expression,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let captured: Vec<(String, XdmValue<N>)> = local_vars
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    Ok(XdmValue::from_function(XdmFunction::inline(
        params.to_vec(),
        body.clone(),
        captured,
    )))
}

fn evaluate_instance_of<'a, N: DataSourceNode<'a> + Clone + 'a>(
    expr: &Expression,
    seq_type: &SequenceType,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let value = evaluate(expr, ctx, local_vars)?;
    let matches = check_sequence_type(&value, seq_type);
    Ok(XdmValue::from_bool(matches))
}

fn evaluate_treat_as<'a, N: DataSourceNode<'a> + Clone + 'a>(
    expr: &Expression,
    seq_type: &SequenceType,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let value = evaluate(expr, ctx, local_vars)?;
    if check_sequence_type(&value, seq_type) {
        Ok(value)
    } else {
        Err(XPath31Error::type_error(format!(
            "Value does not match expected type: {:?}",
            seq_type
        )))
    }
}

fn evaluate_cast_as<'a, N: DataSourceNode<'a> + Clone + 'a>(
    expr: &Expression,
    single_type: &SingleType,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let value = evaluate(expr, ctx, local_vars)?;

    if value.is_empty() {
        return if single_type.optional {
            Ok(XdmValue::empty())
        } else {
            Err(XPath31Error::type_error(
                "Cannot cast empty sequence to non-optional type",
            ))
        };
    }

    let item = value.single().ok_or_else(|| {
        XPath31Error::type_error("cast as requires a single item or empty sequence")
    })?;

    cast_item_to_type(item, single_type)
}

fn evaluate_castable_as<'a, N: DataSourceNode<'a> + Clone + 'a>(
    expr: &Expression,
    single_type: &SingleType,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let value = evaluate(expr, ctx, local_vars)?;

    if value.is_empty() {
        return Ok(XdmValue::from_bool(single_type.optional));
    }

    let item = match value.single() {
        Some(i) => i,
        None => return Ok(XdmValue::from_bool(false)),
    };

    let castable = cast_item_to_type::<N>(item, single_type).is_ok();
    Ok(XdmValue::from_bool(castable))
}

fn check_sequence_type<N: Clone>(value: &XdmValue<N>, seq_type: &SequenceType) -> bool {
    let items = value.items();
    let len = items.len();

    let occurrence_ok = match seq_type.occurrence {
        OccurrenceIndicator::ExactlyOne => len == 1,
        OccurrenceIndicator::ZeroOrOne => len <= 1,
        OccurrenceIndicator::ZeroOrMore => true,
        OccurrenceIndicator::OneOrMore => len >= 1,
    };

    if !occurrence_ok {
        return false;
    }

    items
        .iter()
        .all(|item| check_item_type(item, &seq_type.item_type))
}

fn check_item_type<N: Clone>(item: &XdmItem<N>, item_type: &ItemType) -> bool {
    match item_type {
        ItemType::Item => true,

        ItemType::AtomicOrUnion(qname) => {
            if let XdmItem::Atomic(atomic) = item {
                match_atomic_type(atomic, qname)
            } else {
                false
            }
        }

        ItemType::KindTest(kind_test) => {
            if let XdmItem::Node(_) = item {
                matches!(
                    kind_test,
                    KindTest::AnyKindTest
                        | KindTest::Element(_, _)
                        | KindTest::Attribute(_, _)
                        | KindTest::TextTest
                        | KindTest::CommentTest
                        | KindTest::PITest(_)
                        | KindTest::Document(_)
                        | KindTest::SchemaElement(_)
                        | KindTest::SchemaAttribute(_)
                        | KindTest::NamespaceNodeTest
                )
            } else {
                false
            }
        }

        ItemType::FunctionTest(_, _) => item.is_function(),
        ItemType::MapTest(_, _) => item.is_map(),
        ItemType::ArrayTest(_) => item.is_array(),
        ItemType::ParenthesizedItemType(inner) => check_item_type(item, inner),
    }
}

fn match_atomic_type(atomic: &AtomicValue, type_name: &QName) -> bool {
    match type_name.local_part.as_str() {
        "anyAtomicType" => true,
        "string" => matches!(atomic, AtomicValue::String(_)),
        "boolean" => matches!(atomic, AtomicValue::Boolean(_)),
        "integer" | "int" | "long" | "short" | "byte" => matches!(atomic, AtomicValue::Integer(_)),
        "decimal" => matches!(atomic, AtomicValue::Decimal(_) | AtomicValue::Integer(_)),
        "double" | "float" => matches!(atomic, AtomicValue::Double(_)),
        "numeric" => atomic.is_numeric(),
        "date" => matches!(atomic, AtomicValue::Date(_)),
        "dateTime" => matches!(atomic, AtomicValue::DateTime(_)),
        "time" => matches!(atomic, AtomicValue::Time(_)),
        "duration" | "dayTimeDuration" | "yearMonthDuration" => {
            matches!(atomic, AtomicValue::Duration(_))
        }
        "QName" => matches!(atomic, AtomicValue::QName { .. }),
        "untypedAtomic" => matches!(atomic, AtomicValue::UntypedAtomic(_)),
        _ => false,
    }
}

fn cast_item_to_type<N: Clone>(
    item: &XdmItem<N>,
    single_type: &SingleType,
) -> Result<XdmValue<N>, XPath31Error> {
    let atomic = match item {
        XdmItem::Atomic(a) => a,
        XdmItem::Node(_) => {
            return Err(XPath31Error::type_error(
                "Cannot cast node to atomic type directly",
            ));
        }
        _ => {
            return Err(XPath31Error::type_error(
                "Cannot cast non-atomic item to atomic type",
            ));
        }
    };

    let target = single_type.type_name.local_part.as_str();
    let result = cast_atomic_to_type(atomic, target)?;
    Ok(XdmValue::from_atomic(result))
}

fn cast_atomic_to_type(
    value: &AtomicValue,
    target_type: &str,
) -> Result<AtomicValue, XPath31Error> {
    match target_type {
        "string" => Ok(AtomicValue::String(value.to_string_value())),

        "boolean" => {
            let b = match value {
                AtomicValue::Boolean(b) => *b,
                AtomicValue::String(s) | AtomicValue::UntypedAtomic(s) => match s.as_str() {
                    "true" | "1" => true,
                    "false" | "0" => false,
                    _ => {
                        return Err(XPath31Error::type_error(format!(
                            "Cannot cast '{}' to boolean",
                            s
                        )));
                    }
                },
                AtomicValue::Integer(i) => *i != 0,
                AtomicValue::Double(d) => *d != 0.0 && !d.is_nan(),
                AtomicValue::Decimal(d) => !d.is_zero(),
                _ => return Err(XPath31Error::type_error("Cannot cast to boolean")),
            };
            Ok(AtomicValue::Boolean(b))
        }

        "integer" | "int" | "long" | "short" | "byte" => {
            let i = value.to_integer().ok_or_else(|| {
                XPath31Error::type_error(format!(
                    "Cannot cast '{}' to integer",
                    value.to_string_value()
                ))
            })?;
            Ok(AtomicValue::Integer(i))
        }

        "double" | "float" => {
            let d = value.to_double();
            if d.is_nan() && !matches!(value, AtomicValue::Double(_)) {
                Err(XPath31Error::type_error(format!(
                    "Cannot cast '{}' to double",
                    value.to_string_value()
                )))
            } else {
                Ok(AtomicValue::Double(d))
            }
        }

        "decimal" => {
            let s = value.to_string_value();
            let d: rust_decimal::Decimal = s
                .parse()
                .map_err(|_| XPath31Error::type_error(format!("Cannot cast '{}' to decimal", s)))?;
            Ok(AtomicValue::Decimal(d))
        }

        "untypedAtomic" => Ok(AtomicValue::UntypedAtomic(value.to_string_value())),

        "date" => {
            let s = value.to_string_value();
            match value {
                AtomicValue::Date(_) => Ok(AtomicValue::Date(s)),
                AtomicValue::DateTime(dt) => {
                    if let Some(parsed) = crate::functions::datetime::DateTime::parse(dt) {
                        let date = crate::functions::datetime::Date {
                            year: parsed.year,
                            month: parsed.month,
                            day: parsed.day,
                            timezone: parsed.timezone,
                        };
                        Ok(AtomicValue::Date(date.to_string()))
                    } else {
                        Err(XPath31Error::type_error(format!(
                            "Cannot cast '{}' to date",
                            s
                        )))
                    }
                }
                _ => {
                    if crate::functions::datetime::Date::parse(&s).is_some() {
                        Ok(AtomicValue::Date(s))
                    } else {
                        Err(XPath31Error::type_error(format!(
                            "Cannot cast '{}' to date: invalid ISO-8601 format",
                            s
                        )))
                    }
                }
            }
        }

        "dateTime" => {
            let s = value.to_string_value();
            match value {
                AtomicValue::DateTime(_) => Ok(AtomicValue::DateTime(s)),
                AtomicValue::Date(d) => {
                    if let Some(parsed) = crate::functions::datetime::Date::parse(d) {
                        let dt = crate::functions::datetime::DateTime {
                            year: parsed.year,
                            month: parsed.month,
                            day: parsed.day,
                            hour: 0,
                            minute: 0,
                            second: 0.0,
                            timezone: parsed.timezone,
                        };
                        Ok(AtomicValue::DateTime(dt.to_string()))
                    } else {
                        Err(XPath31Error::type_error(format!(
                            "Cannot cast '{}' to dateTime",
                            s
                        )))
                    }
                }
                _ => {
                    if crate::functions::datetime::DateTime::parse(&s).is_some() {
                        Ok(AtomicValue::DateTime(s))
                    } else {
                        Err(XPath31Error::type_error(format!(
                            "Cannot cast '{}' to dateTime: invalid ISO-8601 format",
                            s
                        )))
                    }
                }
            }
        }

        "time" => {
            let s = value.to_string_value();
            match value {
                AtomicValue::Time(_) => Ok(AtomicValue::Time(s)),
                AtomicValue::DateTime(dt) => {
                    if let Some(parsed) = crate::functions::datetime::DateTime::parse(dt) {
                        let time = crate::functions::datetime::Time {
                            hour: parsed.hour,
                            minute: parsed.minute,
                            second: parsed.second,
                            timezone: parsed.timezone,
                        };
                        Ok(AtomicValue::Time(time.to_string()))
                    } else {
                        Err(XPath31Error::type_error(format!(
                            "Cannot cast '{}' to time",
                            s
                        )))
                    }
                }
                _ => {
                    if crate::functions::datetime::Time::parse(&s).is_some() {
                        Ok(AtomicValue::Time(s))
                    } else {
                        Err(XPath31Error::type_error(format!(
                            "Cannot cast '{}' to time: invalid ISO-8601 format",
                            s
                        )))
                    }
                }
            }
        }

        _ => Err(XPath31Error::type_error(format!(
            "Unknown or unsupported cast target type: {}",
            target_type
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_expression;
    use petty_xpath1::tests::{MockNode, create_test_tree};

    fn eval_str(expr: &str) -> XdmValue<MockNode<'static>> {
        let parsed = parse_expression(expr).unwrap();
        let vars = HashMap::new();
        static TREE: std::sync::OnceLock<petty_xpath1::tests::MockTree<'static>> =
            std::sync::OnceLock::new();
        let _tree = TREE.get_or_init(create_test_tree);
        let ctx: EvaluationContext<'_, '_, MockNode<'static>> =
            EvaluationContext::new(None, None, &vars);
        evaluate(&parsed, &ctx, &HashMap::new()).unwrap()
    }

    #[test]
    fn test_let_simple() {
        let result = eval_str("let $x := 5 return $x * 2");
        assert_eq!(result.to_double(), 10.0);
    }

    #[test]
    fn test_let_chained() {
        let result = eval_str("let $x := 3, $y := 4 return $x + $y");
        assert_eq!(result.to_double(), 7.0);
    }

    #[test]
    fn test_if_true() {
        let result = eval_str("if (1 = 1) then 'yes' else 'no'");
        assert_eq!(result.to_string_value(), "yes");
    }

    #[test]
    fn test_if_false() {
        let result = eval_str("if (1 = 2) then 'yes' else 'no'");
        assert_eq!(result.to_string_value(), "no");
    }

    #[test]
    fn test_for_simple() {
        let result = eval_str("for $i in 1 to 3 return $i * 2");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_quantified_some() {
        let result = eval_str("some $x in (1, 2, 3) satisfies $x > 2");
        assert!(result.effective_boolean_value());
    }

    #[test]
    fn test_quantified_every() {
        let result = eval_str("every $x in (1, 2, 3) satisfies $x > 0");
        assert!(result.effective_boolean_value());

        let result = eval_str("every $x in (1, 2, 3) satisfies $x > 2");
        assert!(!result.effective_boolean_value());
    }

    #[test]
    fn test_range() {
        let result = eval_str("1 to 5");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_string_concat_operator() {
        let result = eval_str("'a' || 'b' || 'c'");
        assert_eq!(result.to_string_value(), "abc");
    }

    #[test]
    fn test_map_constructor() {
        let result = eval_str("map { 'a': 1, 'b': 2 }");
        assert!(result.first().unwrap().is_map());
    }

    #[test]
    fn test_map_lookup() {
        let result = eval_str("map { 'a': 1, 'b': 2 }?a");
        assert_eq!(result.to_double(), 1.0);
    }

    #[test]
    fn test_array_constructor() {
        let result = eval_str("[1, 2, 3]");
        if let Some(XdmItem::Array(arr)) = result.first() {
            assert_eq!(arr.size(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_array_lookup() {
        let result = eval_str("[10, 20, 30]?2");
        assert_eq!(result.to_double(), 20.0);
    }

    #[test]
    fn test_simple_map() {
        let result = eval_str("(1, 2, 3) ! (. * 2)");
        assert_eq!(result.len(), 3);
        let items: Vec<f64> = result
            .items()
            .iter()
            .map(|i| {
                if let XdmItem::Atomic(AtomicValue::Double(d)) = i {
                    *d
                } else {
                    0.0
                }
            })
            .collect();
        assert_eq!(items, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_filter_expr_with_boolean_predicate() {
        let result = eval_str("(1, 2, 3, 4, 5)[. > 3]");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_expr_with_positional_predicate() {
        let result = eval_str("(1, 2, 3)[2]");
        assert_eq!(result.to_double(), 2.0);
    }

    #[test]
    fn test_filter_expr_range_with_predicate() {
        let result = eval_str("(1 to 10)[. > 5]");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_filter_expr_strings() {
        let result = eval_str("('a', 'b', 'c')[. = 'b']");
        assert_eq!(result.to_string_value(), "b");
    }

    #[test]
    fn test_filter_expr_multiple_predicates() {
        let result = eval_str("(1 to 20)[. > 5][. < 15]");
        assert_eq!(result.len(), 9);
    }

    #[test]
    fn test_arithmetic_precedence() {
        let result = eval_str("1 + 2 * 3");
        assert_eq!(result.to_double(), 7.0);
    }

    #[test]
    fn test_comparison() {
        assert!(eval_str("5 < 10").effective_boolean_value());
        assert!(!eval_str("10 < 5").effective_boolean_value());
        assert!(eval_str("5 = 5").effective_boolean_value());
        assert!(!eval_str("5 != 5").effective_boolean_value());
    }

    #[test]
    fn test_map_as_function() {
        let result = eval_str("let $m := map{'a': 1, 'b': 2} return $m('a')");
        assert_eq!(result.to_double(), 1.0);
    }

    #[test]
    fn test_array_as_function() {
        let result = eval_str("let $a := [10, 20, 30] return $a(2)");
        assert_eq!(result.to_double(), 20.0);
    }

    #[test]
    fn test_inline_function_call() {
        let result = eval_str("let $f := function($x) { $x * 2 } return $f(5)");
        assert_eq!(result.to_double(), 10.0);
    }

    #[test]
    fn test_logical_operators() {
        assert!(eval_str("1 = 1 and 2 = 2").effective_boolean_value());
        assert!(!eval_str("1 = 1 and 1 = 2").effective_boolean_value());
        assert!(eval_str("1 = 1 or 1 = 2").effective_boolean_value());
        assert!(!eval_str("1 = 2 or 2 = 3").effective_boolean_value());
    }

    #[test]
    fn test_instance_of_integer() {
        assert!(eval_str("5 instance of xs:integer").effective_boolean_value());
        assert!(!eval_str("'hello' instance of xs:integer").effective_boolean_value());
    }

    #[test]
    fn test_instance_of_string() {
        assert!(eval_str("'hello' instance of xs:string").effective_boolean_value());
        assert!(!eval_str("42 instance of xs:string").effective_boolean_value());
    }

    #[test]
    fn test_instance_of_with_occurrence() {
        assert!(eval_str("(1, 2, 3) instance of xs:integer+").effective_boolean_value());
        assert!(!eval_str("(1, 2, 3) instance of xs:integer").effective_boolean_value());
        assert!(eval_str("() instance of xs:integer*").effective_boolean_value());
        assert!(!eval_str("() instance of xs:integer+").effective_boolean_value());
    }

    #[test]
    fn test_cast_as_string() {
        let result = eval_str("42 cast as xs:string");
        assert_eq!(result.to_string_value(), "42");
    }

    #[test]
    fn test_cast_as_integer() {
        let result = eval_str("'123' cast as xs:integer");
        assert_eq!(result.to_double(), 123.0);
    }

    #[test]
    fn test_cast_as_boolean() {
        assert!(eval_str("'true' cast as xs:boolean").effective_boolean_value());
        assert!(!eval_str("'false' cast as xs:boolean").effective_boolean_value());
        assert!(eval_str("1 cast as xs:boolean").effective_boolean_value());
        assert!(!eval_str("0 cast as xs:boolean").effective_boolean_value());
    }

    #[test]
    fn test_castable_as() {
        assert!(eval_str("'123' castable as xs:integer").effective_boolean_value());
        assert!(!eval_str("'hello' castable as xs:integer").effective_boolean_value());
        assert!(eval_str("42 castable as xs:string").effective_boolean_value());
    }

    #[test]
    fn test_treat_as() {
        let result = eval_str("5 treat as xs:integer");
        assert_eq!(result.to_double(), 5.0);
    }
}
