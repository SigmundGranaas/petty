//! The evaluation engine for executing a parsed XPath AST against a generic `DataSourceNode`.

use super::ast::{Axis, Expression, LocationPath, NodeTest, NodeTypeTest, Step, UnaryOperator};
use super::functions::{self, FunctionRegistry};
use super::{axes, operators};
use crate::datasource::{DataSourceNode, NodeType};
use crate::error::XPathError;
use std::collections::{HashMap, HashSet};
use std::fmt;
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
            XPathValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            XPathValue::NodeSet(nodes) => {
                let s = nodes.first().map(|n| n.string_value()).unwrap_or_default();
                s.trim().parse().unwrap_or(f64::NAN)
            }
        }
    }
}

impl<'a, N: DataSourceNode<'a>> fmt::Display for XPathValue<N> {
    /// Coerces the XPath value to a string as per XPath 1.0 rules.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XPathValue::NodeSet(nodes) => write!(
                f,
                "{}",
                nodes.first().map(|n| n.string_value()).unwrap_or_default()
            ),
            XPathValue::String(s) => write!(f, "{}", s),
            XPathValue::Number(n) => write!(f, "{}", n),
            XPathValue::Boolean(b) => write!(f, "{}", b),
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
    /// Read-only access to the pre-computed key indexes.
    pub key_indexes: &'d HashMap<String, HashMap<String, Vec<N>>>,
    /// If true, enables strict error checking.
    pub strict: bool,
    _marker: PhantomData<&'a ()>,
}

impl<'a, 'd, N: DataSourceNode<'a>> EvaluationContext<'a, 'd, N> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context_node: N,
        root_node: N,
        functions: &'d FunctionRegistry,
        context_position: usize,
        context_size: usize,
        variables: &'d HashMap<String, XPathValue<N>>,
        key_indexes: &'d HashMap<String, HashMap<String, Vec<N>>>,
        strict: bool,
    ) -> Self {
        Self {
            context_node,
            root_node,
            functions,
            context_position,
            context_size,
            variables,
            key_indexes,
            strict,
            _marker: PhantomData,
        }
    }
}

/// Evaluates a compiled expression and returns a concrete `XPathValue`.
pub fn evaluate<'a, N>(
    expr: &Expression,
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XPathValue<N>, XPathError>
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
            if e_ctx.strict && !e_ctx.variables.contains_key(name) {
                return Err(XPathError::TypeError(format!(
                    "Reference to undeclared variable: ${}",
                    name
                )));
            }
            Ok(e_ctx
                .variables
                .get(name)
                .cloned()
                .unwrap_or(XPathValue::String("".to_string())))
        }
        Expression::FunctionCall { name, args } => {
            let mut evaluated_args = Vec::with_capacity(args.len());
            for arg in args {
                evaluated_args.push(evaluate(arg, e_ctx)?);
            }
            Ok(functions::evaluate_function(name, evaluated_args, e_ctx)?)
        }
        Expression::BinaryOp { left, op, right } => {
            let left_val = evaluate(left, e_ctx)?;
            let right_val = evaluate(right, e_ctx)?;
            operators::evaluate(*op, left_val, right_val)
        }
        Expression::UnaryOp { op, expr } => {
            let val = evaluate(expr, e_ctx)?;
            match op {
                UnaryOperator::Minus => Ok(XPathValue::Number(-val.to_number())),
            }
        }
    }
}

fn evaluate_location_path<'a, N>(
    path: &LocationPath,
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<Vec<N>, XPathError>
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
) -> Result<Vec<N>, XPathError>
where
    N: DataSourceNode<'a> + 'a,
{
    // Handle special abbreviated step '.' which means the context node set itself.
    if step.axis == Axis::SelfAxis && step.node_test == NodeTest::Name(".".to_string()) {
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
        match axis {
            Axis::Child => axes::collect_child_nodes(node, &mut seen, &mut result_nodes),
            Axis::Attribute => axes::collect_attribute_nodes(node, &mut seen, &mut result_nodes),
            Axis::Descendant => axes::collect_descendant_nodes(node, &mut seen, &mut result_nodes),
            Axis::DescendantOrSelf => {
                axes::collect_descendant_or_self_nodes(node, &mut seen, &mut result_nodes)
            }
            Axis::Parent => axes::collect_parent_nodes(node, &mut seen, &mut result_nodes),
            Axis::Ancestor => axes::collect_ancestor_nodes(node, &mut seen, &mut result_nodes),
            Axis::SelfAxis => axes::collect_self_nodes(node, &mut seen, &mut result_nodes),
            Axis::FollowingSibling => {
                axes::collect_following_sibling_nodes(node, &mut seen, &mut result_nodes)
            }
            Axis::PrecedingSibling => {
                axes::collect_preceding_sibling_nodes(node, &mut seen, &mut result_nodes)
            }
            Axis::Following => axes::collect_following_nodes(node, &mut seen, &mut result_nodes),
            Axis::Preceding => axes::collect_preceding_nodes(node, &mut seen, &mut result_nodes),
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
        .filter(|&node| match test {
            NodeTest::Wildcard => match axis {
                Axis::Attribute => node.node_type() == NodeType::Attribute,
                _ => node.node_type() == NodeType::Element,
            },
            NodeTest::Name(name_to_test) => node
                .name()
                .is_some_and(|q_name| q_name.local_part == name_to_test),
            NodeTest::NodeType(ntt) => match ntt {
                NodeTypeTest::Text => node.node_type() == NodeType::Text,
                NodeTypeTest::Comment => node.node_type() == NodeType::Comment,
                NodeTypeTest::ProcessingInstruction => {
                    node.node_type() == NodeType::ProcessingInstruction
                }
                NodeTypeTest::Node => true,
            },
        })
        .copied()
        .collect()
}

/// Stage 3: Filters a set of nodes by applying a series of predicates.
fn apply_predicates<'a, N>(
    nodes: &[N],
    predicates: &[Expression],
    e_ctx: &EvaluationContext<'a, '_, N>,
) -> Result<Vec<N>, XPathError>
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
                e_ctx.key_indexes, // Pass through the key indexes
                e_ctx.strict,      // Propagate strict mode
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
    use crate::datasource::tests::{MockNode, create_test_tree};
    use std::collections::HashMap;

    fn create_test_eval_context<'a, 'd>(
        tree: &'a crate::datasource::tests::MockTree<'a>,
        functions: &'d FunctionRegistry,
        vars: &'d HashMap<String, XPathValue<MockNode<'a>>>,
        keys: &'d HashMap<String, HashMap<String, Vec<MockNode<'a>>>>,
    ) -> EvaluationContext<'a, 'd, MockNode<'a>> {
        let root = MockNode { id: 0, tree };
        EvaluationContext::new(root, root, functions, 1, 1, vars, keys, false)
    }

    #[test]
    fn test_pipeline_functions_individually() {
        let tree = create_test_tree();
        let root = MockNode { id: 0, tree: &tree };
        let para = MockNode { id: 1, tree: &tree };
        let attr = MockNode { id: 2, tree: &tree };
        let text = MockNode { id: 4, tree: &tree };

        // Test collect_axis_nodes
        let children = collect_axis_nodes(Axis::Child, &[root]);
        assert_eq!(children.len(), 5);
        let attributes = collect_axis_nodes(Axis::Attribute, &[para]);
        assert_eq!(attributes.len(), 2);
        let ancestors = collect_axis_nodes(Axis::Ancestor, &[text]);
        assert_eq!(ancestors, vec![para, root]);

        // Test filter_by_node_test
        let all_nodes = vec![root, para, attr, text];
        let elements = filter_by_node_test(&all_nodes, &NodeTest::Wildcard, Axis::Child);
        assert_eq!(elements, vec![para]);
        let para_nodes =
            filter_by_node_test(&all_nodes, &NodeTest::Name("para".to_string()), Axis::Child);
        assert_eq!(para_nodes, vec![para]);
        let text_nodes = filter_by_node_test(
            &all_nodes,
            &NodeTest::NodeType(NodeTypeTest::Text),
            Axis::Child,
        );
        assert_eq!(text_nodes, vec![text]);

        // Test apply_predicates (positional)
        let funcs = FunctionRegistry::default();
        let vars = HashMap::new();
        let keys = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars, &keys);
        // FIX: Parse only the expression within the predicate.
        let predicate_expr = crate::parser::parse_expression("position()=2").unwrap();
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
        let keys = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars, &keys);

        let expr = crate::parser::parse_expression("child::para[@id='p1']").unwrap();
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
        let keys = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars, &keys);

        let expr = crate::parser::parse_expression("child::para[1]").unwrap();
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
        let keys = HashMap::new();
        let e_ctx = create_test_eval_context(&tree, &funcs, &vars, &keys);

        let expr = crate::parser::parse_expression("child::para[position()=1]").unwrap();
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
        let keys = HashMap::new();

        let mut vars = HashMap::new();
        vars.insert(
            "myVar".to_string(),
            XPathValue::String("test-value".to_string()),
        );

        let e_ctx = create_test_eval_context(&tree, &funcs, &vars, &keys);

        let expr = crate::parser::parse_expression("$myVar").unwrap();
        let result = evaluate(&expr, &e_ctx).unwrap();
        assert_eq!(result.to_string(), "test-value");
    }

    #[test]
    fn test_path_from_variable_node_set() {
        let tree = create_test_tree();
        let funcs = FunctionRegistry::default();
        let keys = HashMap::new();
        let mut vars = HashMap::new();

        // Put the <para> node (id 1) into a variable
        let para_node = MockNode { id: 1, tree: &tree };
        vars.insert(
            "para_node".to_string(),
            XPathValue::NodeSet(vec![para_node]),
        );

        let e_ctx = create_test_eval_context(&tree, &funcs, &vars, &keys);

        // Select the text() node from the node in the variable
        let expr = crate::parser::parse_expression("$para_node/text()").unwrap();
        let result = evaluate(&expr, &e_ctx).unwrap();

        if let XPathValue::NodeSet(nodes) = result {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].id, 4); // id of the text node "Hello"
            assert_eq!(nodes[0].string_value(), "Hello");
        } else {
            panic!("Expected a NodeSet");
        }
    }
}
