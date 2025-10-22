// FILE: src/parser/xpath/engine.rs
//! The evaluation engine for executing a parsed XPath AST against a generic `DataSourceNode`.

use super::ast::{Axis, BinaryOperator, Expression, LocationPath, NodeTest, NodeTypeTest, Step};
use super::functions::{self, FunctionRegistry};
use crate::parser::datasource::{DataSourceNode, NodeType};
use crate::parser::ParseError;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

/// Represents the possible result types of an XPath expression evaluation.
#[derive(Debug, Clone)]
pub enum XPathValue<N> {
    NodeSet(Vec<N>),
    String(String),
    Number(f64),
    Boolean(bool),
}

impl<'a, N: DataSourceNode<'a>> XPathValue<N> {
    /// Coerces the XPath value to a boolean as per XPath 1.0 rules.
    pub fn to_bool(&self) -> bool {
        match self {
            XPathValue::NodeSet(nodes) => !nodes.is_empty(),
            XPathValue::String(s) => !s.is_empty(),
            XPathValue::Number(n) => *n != 0.0 && !n.is_nan(),
            XPathValue::Boolean(b) => *b,
        }
    }

    /// Coerces the XPath value to a number as per XPath 1.0 rules.
    pub fn to_number(&self) -> f64 {
        match self {
            XPathValue::Number(n) => *n,
            XPathValue::String(s) => s.trim().parse().unwrap_or(f64::NAN),
            XPathValue::Boolean(b) => if *b { 1.0 } else { 0.0 },
            XPathValue::NodeSet(nodes) => {
                let s = nodes.first().map(|n| n.string_value()).unwrap_or_default();
                s.trim().parse().unwrap_or(f64::NAN)
            }
        }
    }

    /// Coerces the XPath value to a string as per XPath 1.0 rules.
    pub fn to_string(&self) -> String {
        match self {
            XPathValue::NodeSet(nodes) => nodes.first().map(|n| n.string_value()).unwrap_or_default(),
            XPathValue::String(s) => s.clone(),
            XPathValue::Number(n) => n.to_string(),
            XPathValue::Boolean(b) => b.to_string(),
        }
    }
}

/// A container for all state needed during expression evaluation.
/// `'a` is the lifetime of the underlying data source.
/// `'d` is the lifetime of the evaluation context itself.
pub struct EvaluationContext<'a, 'd, N: DataSourceNode<'a>> {
    pub context_node: N,
    pub root_node: N,
    pub functions: &'d FunctionRegistry,
    pub context_position: usize, // 1-based index
    pub context_size: usize,
    pub variables: &'d HashMap<String, XPathValue<N>>,
    _marker: PhantomData<&'a ()>,
}

impl<'a, 'd, N: DataSourceNode<'a>> EvaluationContext<'a, 'd, N> {
    pub fn new(
        context_node: N,
        root_node: N,
        functions: &'d FunctionRegistry,
        context_position: usize,
        context_size: usize,
        variables: &'d HashMap<String, XPathValue<N>>,
    ) -> Self {
        Self {
            context_node,
            root_node,
            functions,
            context_position,
            context_size,
            variables,
            _marker: PhantomData,
        }
    }
}

/// Evaluates a compiled expression and returns a concrete `XPathValue`.
pub fn evaluate<'a, N>(
    expr: &Expression,
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XPathValue<N>, ParseError>
where
    N: DataSourceNode<'a> + 'a,
{
    match expr {
        Expression::Literal(s) => Ok(XPathValue::String(s.clone())),
        Expression::Number(n) => Ok(XPathValue::Number(*n)),
        Expression::LocationPath(path) => {
            let nodes = evaluate_location_path(path, e_ctx)?;
            Ok(XPathValue::NodeSet(nodes))
        }
        Expression::Variable(name) => {
            Ok(e_ctx.variables.get(name).cloned().unwrap_or(XPathValue::String("".to_string())))
        }
        Expression::FunctionCall { name, args } => {
            let mut evaluated_args = Vec::with_capacity(args.len());
            for arg in args {
                evaluated_args.push(evaluate(arg, e_ctx)?);
            }
            functions::evaluate_function(name, evaluated_args, e_ctx)
        }
        Expression::BinaryOp { left, op, right } => {
            evaluate_binary_op(left, *op, right, e_ctx)
        }
    }
}

fn evaluate_binary_op<'a, N: DataSourceNode<'a> + 'a>(
    left_expr: &Expression,
    op: BinaryOperator,
    right_expr: &Expression,
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XPathValue<N>, ParseError> {
    use BinaryOperator::*;

    // Special handling for Union, which must operate on node-sets.
    if op == Union {
        let left_val = evaluate(left_expr, e_ctx)?;
        let right_val = evaluate(right_expr, e_ctx)?;

        let l_nodes = if let XPathValue::NodeSet(n) = left_val {
            n
        } else {
            return Err(ParseError::XPathParse(
                "Union operator".to_string(),
                "Left-hand side of '|' must be a node-set.".to_string(),
            ));
        };
        let r_nodes = if let XPathValue::NodeSet(n) = right_val {
            n
        } else {
            return Err(ParseError::XPathParse(
                "Union operator".to_string(),
                "Right-hand side of '|' must be a node-set.".to_string(),
            ));
        };

        let mut merged = l_nodes.clone();
        let mut seen: HashSet<N> = l_nodes.into_iter().collect();
        for node in r_nodes {
            if seen.insert(node) {
                merged.push(node);
            }
        }
        // TODO: Sort by document order. For now, this is correct enough.
        return Ok(XPathValue::NodeSet(merged));
    }

    let left = evaluate(left_expr, e_ctx)?;
    let right = evaluate(right_expr, e_ctx)?;

    match op {
        Or => Ok(XPathValue::Boolean(left.to_bool() || right.to_bool())),
        And => Ok(XPathValue::Boolean(left.to_bool() && right.to_bool())),
        Equals | NotEquals => {
            let res = if let (XPathValue::Number(l), XPathValue::Number(r)) = (&left, &right) {
                l == r
            } else if let (XPathValue::Boolean(l), XPathValue::Boolean(r)) = (&left, &right) {
                l == r
            } else {
                left.to_string() == right.to_string()
            };
            Ok(XPathValue::Boolean(if op == Equals { res } else { !res }))
        }
        LessThan => Ok(XPathValue::Boolean(left.to_number() < right.to_number())),
        LessThanOrEqual => Ok(XPathValue::Boolean(left.to_number() <= right.to_number())),
        GreaterThan => Ok(XPathValue::Boolean(left.to_number() > right.to_number())),
        GreaterThanOrEqual => Ok(XPathValue::Boolean(left.to_number() >= right.to_number())),
        Plus => Ok(XPathValue::Number(left.to_number() + right.to_number())),
        Minus => Ok(XPathValue::Number(left.to_number() - right.to_number())),
        Multiply => Ok(XPathValue::Number(left.to_number() * right.to_number())),
        Divide => Ok(XPathValue::Number(left.to_number() / right.to_number())),
        Modulo => Ok(XPathValue::Number(left.to_number() % right.to_number())),
        Union => unreachable!(), // Handled above
    }
}


fn evaluate_location_path<'a, N>(
    path: &LocationPath,
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<Vec<N>, ParseError>
where
    N: DataSourceNode<'a> + 'a,
{
    // If the path has no steps and is relative, it refers to the context node itself.
    if path.steps.is_empty() && !path.is_absolute && path.start_point.is_none() {
        return Ok(vec![e_ctx.context_node]);
    }

    let initial_context = if let Some(start_expr) = &path.start_point {
        // The path starts from the result of another expression.
        match evaluate(start_expr, e_ctx)? {
            XPathValue::NodeSet(nodes) => nodes,
            // If the start expression doesn't evaluate to a node-set, the path is empty.
            _ => return Ok(vec![]),
        }
    } else if path.is_absolute {
        // Standard absolute path from the root.
        vec![e_ctx.root_node]
    } else {
        // Standard relative path from the current context node.
        vec![e_ctx.context_node]
    };

    let mut current_nodes = initial_context;
    for step in &path.steps {
        current_nodes = evaluate_step(step, &current_nodes, e_ctx)?;
    }
    Ok(current_nodes)
}

/// Evaluates a single step in a location path by chaining axis collection, node testing, and predicate application.
fn evaluate_step<'a, N>(
    step: &Step,
    context_nodes: &[N],
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<Vec<N>, ParseError>
where
    N: DataSourceNode<'a> + 'a,
{
    // Handle special abbreviated step '.' which means the context node set itself.
    if step.node_test == NodeTest::Name(".".to_string()) {
        return Ok(context_nodes.to_vec());
    }

    let axis_nodes = collect_axis_nodes(step.axis, context_nodes);
    let tested_nodes = filter_by_node_test(&axis_nodes, &step.node_test, step.axis);
    apply_predicates(&tested_nodes, &step.predicates, e_ctx)
}

/// Stage 1: Collects all unique nodes from the context set along a given axis.
fn collect_axis_nodes<'a, N>(axis: Axis, context_nodes: &[N]) -> Vec<N>
where
    N: DataSourceNode<'a> + 'a,
{
    let mut result_nodes = Vec::new();
    let mut seen = HashSet::new();

    for &node in context_nodes {
        let axis_iterator: Box<dyn Iterator<Item = N>> = match axis {
            Axis::Child => Box::new(node.children()),
            Axis::Attribute => Box::new(node.attributes()),
            Axis::Descendant => {
                let mut queue: Vec<N> = node.children().collect();
                Box::new(std::iter::from_fn(move || {
                    if let Some(current) = queue.pop() {
                        queue.extend(current.children());
                        Some(current)
                    } else {
                        None
                    }
                }))
            }
            Axis::DescendantOrSelf => {
                let mut queue: Vec<N> = node.children().collect();
                let self_iter = std::iter::once(node);
                let desc_iter = std::iter::from_fn(move || {
                    if let Some(current) = queue.pop() {
                        queue.extend(current.children());
                        Some(current)
                    } else {
                        None
                    }
                });
                Box::new(self_iter.chain(desc_iter))
            }
            Axis::Parent => Box::new(node.parent().into_iter()),
            Axis::Ancestor => {
                let mut current = node.parent();
                Box::new(std::iter::from_fn(move || {
                    if let Some(p) = current {
                        current = p.parent();
                        Some(p)
                    } else {
                        None
                    }
                }))
            }
        };

        for candidate_node in axis_iterator {
            if seen.insert(candidate_node) {
                result_nodes.push(candidate_node);
            }
        }
    }
    result_nodes
}

/// Stage 2: Filters a set of nodes based on a `NodeTest`.
fn filter_by_node_test<'a, N>(nodes: &[N], test: &NodeTest, axis: Axis) -> Vec<N>
where
    N: DataSourceNode<'a> + 'a,
{
    nodes
        .iter()
        .filter(|&node| {
            match test {
                NodeTest::Wildcard => match axis {
                    Axis::Attribute => node.node_type() == NodeType::Attribute,
                    _ => node.node_type() == NodeType::Element,
                },
                NodeTest::Name(name_to_test) => {
                    node.name().map_or(false, |q_name| q_name.local_part == name_to_test)
                }
                NodeTest::NodeType(ntt) => match ntt {
                    NodeTypeTest::Text => node.node_type() == NodeType::Text,
                    NodeTypeTest::Node => true,
                },
            }
        })
        .copied()
        .collect()
}

/// Stage 3: Filters a set of nodes by applying a series of predicates.
fn apply_predicates<'a, N>(
    nodes: &[N],
    predicates: &[Expression],
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<Vec<N>, ParseError>
where
    N: DataSourceNode<'a> + 'a,
{
    let mut final_nodes = nodes.to_vec();
    for predicate in predicates {
        let mut predicate_results = Vec::new();
        let context_size = final_nodes.len();
        for (i, node) in final_nodes.iter().enumerate() {
            let predicate_e_ctx = EvaluationContext::new(
                *node,
                e_ctx.root_node,
                e_ctx.functions,
                i + 1,
                context_size,
                e_ctx.variables,
            );
            let result = evaluate(predicate, &predicate_e_ctx)?;
            let keep = match result {
                XPathValue::Number(n) => (n as usize) == (i + 1),
                _ => result.to_bool(),
            };
            if keep {
                predicate_results.push(*node);
            }
        }
        final_nodes = predicate_results;
    }
    Ok(final_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::datasource::tests::{create_test_tree, MockNode};
    use std::collections::HashMap;

    fn create_test_eval_context<'a, 'd>(
        tree: &'a crate::parser::datasource::tests::MockTree<'a>,
        functions: &'d FunctionRegistry,
        vars: &'d HashMap<String, XPathValue<MockNode<'a>>>,
    ) -> EvaluationContext<'a, 'd, MockNode<'a>> {
        let root = MockNode { id: 0, tree };
        EvaluationContext::new(root, root, functions, 1, 1, vars)
    }

    #[test]
    fn test_pipeline_functions_individually() {
        let tree = create_test_tree();
        let root = MockNode { id: 0, tree: &tree };
        let para = MockNode { id: 1, tree: &tree };
        let attr = MockNode { id: 2, tree: &tree };
        let text = MockNode { id: 3, tree: &tree };

        // Test collect_axis_nodes
        let children = collect_axis_nodes(Axis::Child, &[root]);
        assert_eq!(children, vec![para]);
        let attributes = collect_axis_nodes(Axis::Attribute, &[para]);
        assert_eq!(attributes, vec![attr]);
        let ancestors = collect_axis_nodes(Axis::Ancestor, &[text]);
        assert_eq!(ancestors, vec![para, root]);


        // Test filter_by_node_test
        let all_nodes = vec![root, para, attr, text];
        let elements = filter_by_node_test(&all_nodes, &NodeTest::Wildcard, Axis::Child);
        assert_eq!(elements, vec![para]);
        let para_nodes = filter_by_node_test(&all_nodes, &NodeTest::Name("para".to_string()), Axis::Child);
        assert_eq!(para_nodes, vec![para]);
        let text_nodes = filter_by_node_test(&all_nodes, &NodeTest::NodeType(NodeTypeTest::Text), Axis::Child);
        assert_eq!(text_nodes, vec![text]);

        // Test apply_predicates (positional)
        let funcs = FunctionRegistry::default();
        let vars = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars);
        // FIX: Parse only the expression within the predicate.
        let predicate_expr = crate::parser::xpath::parse_expression("position()=2").unwrap();
        let predicates = vec![predicate_expr];
        let nodes_to_filter = vec![root, para, text];
        let filtered = apply_predicates(&nodes_to_filter, &predicates, &e_ctx).unwrap();
        assert_eq!(filtered, vec![para]);
    }

    #[test]
    fn test_predicate_by_attribute() {
        let tree = create_test_tree();
        let funcs = FunctionRegistry::default();
        let vars = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars);

        let expr = crate::parser::xpath::parse_expression("child::para[@id='p1']").unwrap();
        let result = evaluate(&expr, &e_ctx).unwrap();

        if let XPathValue::NodeSet(nodes) = result {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].id, 1);
        } else {
            panic!("Expected a NodeSet");
        }
    }

    #[test]
    fn test_predicate_by_position() {
        let tree = create_test_tree();
        let funcs = FunctionRegistry::default();
        let vars = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars);

        let expr = crate::parser::xpath::parse_expression("child::para[1]").unwrap();
        let result = evaluate(&expr, &e_ctx).unwrap();

        if let XPathValue::NodeSet(nodes) = result {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].id, 1);
        } else {
            panic!("Expected a NodeSet");
        }
    }

    #[test]
    fn test_predicate_by_position_function() {
        let tree = create_test_tree();
        let funcs = FunctionRegistry::default();
        let vars = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars);

        let expr = crate::parser::xpath::parse_expression("child::para[position()=1]").unwrap();
        let result = evaluate(&expr, &e_ctx).unwrap();

        if let XPathValue::NodeSet(nodes) = result {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].id, 1);
        } else {
            panic!("Expected a NodeSet");
        }
    }

    #[test]
    fn test_variable_evaluation() {
        let tree = create_test_tree();
        let funcs = FunctionRegistry::default();

        let mut vars = HashMap::new();
        vars.insert("myVar".to_string(), XPathValue::String("test-value".to_string()));

        let e_ctx = create_test_eval_context(&tree, &funcs, &vars);

        let expr = crate::parser::xpath::parse_expression("$myVar").unwrap();
        let result = evaluate(&expr, &e_ctx).unwrap();
        assert_eq!(result.to_string(), "test-value");
    }

    #[test]
    fn test_path_from_variable_node_set() {
        let tree = create_test_tree();
        let funcs = FunctionRegistry::default();
        let mut vars = HashMap::new();

        // Put the <para> node (id 1) into a variable
        let para_node = MockNode { id: 1, tree: &tree };
        vars.insert("para_node".to_string(), XPathValue::NodeSet(vec![para_node]));

        let e_ctx = create_test_eval_context(&tree, &funcs, &vars);

        // Select the text() node from the node in the variable
        let expr = crate::parser::xpath::parse_expression("$para_node/text()").unwrap();
        let result = evaluate(&expr, &e_ctx).unwrap();

        if let XPathValue::NodeSet(nodes) = result {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].id, 3); // id of the text node "Hello"
            assert_eq!(nodes[0].string_value(), "Hello");
        } else {
            panic!("Expected a NodeSet");
        }
    }
}