//! Contains pure functions for evaluating XPath binary operators.

use super::ast::BinaryOperator;
use super::engine::XPathValue;
use crate::parser::xslt::datasource::DataSourceNode;
use crate::parser::xslt::executor::ExecutionError;

pub fn evaluate<'a, N: DataSourceNode<'a> + 'a>(
    op: BinaryOperator,
    left: XPathValue<N>,
    right: XPathValue<N>,
) -> Result<XPathValue<N>, ExecutionError> {
    use BinaryOperator::*;
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
        Union => evaluate_union(left, right),
    }
}

fn evaluate_union<'a, N: DataSourceNode<'a> + 'a>(
    left: XPathValue<N>,
    right: XPathValue<N>,
) -> Result<XPathValue<N>, ExecutionError> {
    let l_nodes = if let XPathValue::NodeSet(n) = left {
        n
    } else {
        return Err(ExecutionError::TypeError(
            "Left-hand side of '|' must be a node-set.".to_string(),
        ));
    };
    let r_nodes = if let XPathValue::NodeSet(n) = right {
        n
    } else {
        return Err(ExecutionError::TypeError(
            "Right-hand side of '|' must be a node-set.".to_string(),
        ));
    };

    let mut merged = l_nodes;
    merged.extend(r_nodes);
    merged.sort();
    merged.dedup();
    Ok(XPathValue::NodeSet(merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::xslt::datasource::tests::{create_test_tree, MockNode};

    #[test]
    fn test_logical_operators() {
        let left_true = XPathValue::Boolean::<MockNode>(true);
        let right_false = XPathValue::Boolean::<MockNode>(false);
        assert_eq!(
            evaluate(BinaryOperator::Or, left_true.clone(), right_false.clone())
                .unwrap()
                .to_bool(),
            true
        );
        assert_eq!(
            evaluate(BinaryOperator::And, left_true.clone(), right_false.clone())
                .unwrap()
                .to_bool(),
            false
        );
    }

    #[test]
    fn test_arithmetic_operators() {
        let left = XPathValue::Number::<MockNode>(10.0);
        let right = XPathValue::Number::<MockNode>(3.0);
        assert_eq!(
            evaluate(BinaryOperator::Plus, left.clone(), right.clone())
                .unwrap()
                .to_number(),
            13.0
        );
        assert_eq!(
            evaluate(BinaryOperator::Minus, left.clone(), right.clone())
                .unwrap()
                .to_number(),
            7.0
        );
        assert_eq!(
            evaluate(BinaryOperator::Multiply, left.clone(), right.clone())
                .unwrap()
                .to_number(),
            30.0
        );
        assert!((evaluate(BinaryOperator::Divide, left.clone(), right.clone()).unwrap().to_number() - 3.333).abs() < 0.001);
        assert_eq!(
            evaluate(BinaryOperator::Modulo, left.clone(), right.clone())
                .unwrap()
                .to_number(),
            1.0
        );
    }

    #[test]
    fn test_equality_operators() {
        let left_str = XPathValue::String::<MockNode>("hello".to_string());
        let right_str = XPathValue::String::<MockNode>("world".to_string());
        assert_eq!(
            evaluate(BinaryOperator::NotEquals, left_str.clone(), right_str.clone())
                .unwrap()
                .to_bool(),
            true
        );
        assert_eq!(
            evaluate(BinaryOperator::Equals, left_str.clone(), left_str.clone())
                .unwrap()
                .to_bool(),
            true
        );
    }

    #[test]
    fn test_union_operator() {
        let tree = create_test_tree();
        let root = MockNode { id: 0, tree: &tree };
        let para = MockNode { id: 1, tree: &tree };
        let text = MockNode { id: 4, tree: &tree }; // id 3 is now an attribute

        let left = XPathValue::NodeSet(vec![para, root]); // out of order
        let right = XPathValue::NodeSet(vec![para, text]);

        let result = evaluate(BinaryOperator::Union, left, right).unwrap();
        if let XPathValue::NodeSet(nodes) = result {
            assert_eq!(nodes.len(), 3);
            // Check that they are sorted and unique
            assert_eq!(nodes, vec![root, para, text]);
        } else {
            panic!("Expected NodeSet result");
        }
    }
}