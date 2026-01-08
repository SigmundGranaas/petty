use crate::error::XPath31Error;
use crate::types::{AtomicValue, XdmItem, XdmValue};
use petty_xpath1::ast::BinaryOperator;

pub fn evaluate_binary<N: Clone + Eq + std::hash::Hash + Ord>(
    op: BinaryOperator,
    left: XdmValue<N>,
    right: XdmValue<N>,
) -> Result<XdmValue<N>, XPath31Error> {
    match op {
        BinaryOperator::Or => evaluate_or(left, right),
        BinaryOperator::And => evaluate_and(left, right),
        BinaryOperator::Equals => {
            evaluate_comparison(left, right, |ord| ord == std::cmp::Ordering::Equal)
        }
        BinaryOperator::NotEquals => {
            evaluate_comparison(left, right, |ord| ord != std::cmp::Ordering::Equal)
        }
        BinaryOperator::LessThan => {
            evaluate_comparison(left, right, |ord| ord == std::cmp::Ordering::Less)
        }
        BinaryOperator::LessThanOrEqual => {
            evaluate_comparison(left, right, |ord| ord != std::cmp::Ordering::Greater)
        }
        BinaryOperator::GreaterThan => {
            evaluate_comparison(left, right, |ord| ord == std::cmp::Ordering::Greater)
        }
        BinaryOperator::GreaterThanOrEqual => {
            evaluate_comparison(left, right, |ord| ord != std::cmp::Ordering::Less)
        }
        BinaryOperator::Plus => evaluate_arithmetic(left, right, |a, b| a + b),
        BinaryOperator::Minus => evaluate_arithmetic(left, right, |a, b| a - b),
        BinaryOperator::Multiply => evaluate_arithmetic(left, right, |a, b| a * b),
        BinaryOperator::Divide => evaluate_divide(left, right),
        BinaryOperator::Modulo => evaluate_modulo(left, right),
        BinaryOperator::Union => evaluate_union(left, right),
    }
}

pub fn evaluate_binary_with_nodes<'a, N>(
    op: BinaryOperator,
    left: XdmValue<N>,
    right: XdmValue<N>,
) -> Result<XdmValue<N>, XPath31Error>
where
    N: Clone + Eq + std::hash::Hash + Ord + petty_xpath1::DataSourceNode<'a> + 'a,
{
    match op {
        BinaryOperator::Or => evaluate_or(left, right),
        BinaryOperator::And => evaluate_and(left, right),
        BinaryOperator::Equals => {
            evaluate_comparison_with_nodes(left, right, |ord| ord == std::cmp::Ordering::Equal)
        }
        BinaryOperator::NotEquals => {
            evaluate_comparison_with_nodes(left, right, |ord| ord != std::cmp::Ordering::Equal)
        }
        BinaryOperator::LessThan => {
            evaluate_comparison_with_nodes(left, right, |ord| ord == std::cmp::Ordering::Less)
        }
        BinaryOperator::LessThanOrEqual => {
            evaluate_comparison_with_nodes(left, right, |ord| ord != std::cmp::Ordering::Greater)
        }
        BinaryOperator::GreaterThan => {
            evaluate_comparison_with_nodes(left, right, |ord| ord == std::cmp::Ordering::Greater)
        }
        BinaryOperator::GreaterThanOrEqual => {
            evaluate_comparison_with_nodes(left, right, |ord| ord != std::cmp::Ordering::Less)
        }
        BinaryOperator::Plus => evaluate_arithmetic(left, right, |a, b| a + b),
        BinaryOperator::Minus => evaluate_arithmetic(left, right, |a, b| a - b),
        BinaryOperator::Multiply => evaluate_arithmetic(left, right, |a, b| a * b),
        BinaryOperator::Divide => evaluate_divide(left, right),
        BinaryOperator::Modulo => evaluate_modulo(left, right),
        BinaryOperator::Union => evaluate_union(left, right),
    }
}

fn evaluate_or<N: Clone>(
    left: XdmValue<N>,
    right: XdmValue<N>,
) -> Result<XdmValue<N>, XPath31Error> {
    let result = left.effective_boolean_value() || right.effective_boolean_value();
    Ok(XdmValue::from_bool(result))
}

fn evaluate_and<N: Clone>(
    left: XdmValue<N>,
    right: XdmValue<N>,
) -> Result<XdmValue<N>, XPath31Error> {
    let result = left.effective_boolean_value() && right.effective_boolean_value();
    Ok(XdmValue::from_bool(result))
}

fn evaluate_comparison<N: Clone, F>(
    left: XdmValue<N>,
    right: XdmValue<N>,
    predicate: F,
) -> Result<XdmValue<N>, XPath31Error>
where
    F: Fn(std::cmp::Ordering) -> bool,
{
    let left_items = left.items();
    let right_items = right.items();

    for l_item in left_items {
        for r_item in right_items {
            if let Some(ord) = compare_items(l_item, r_item)
                && predicate(ord)
            {
                return Ok(XdmValue::from_bool(true));
            }
        }
    }

    Ok(XdmValue::from_bool(false))
}

fn evaluate_comparison_with_nodes<'a, N, F>(
    left: XdmValue<N>,
    right: XdmValue<N>,
    predicate: F,
) -> Result<XdmValue<N>, XPath31Error>
where
    N: Clone + petty_xpath1::DataSourceNode<'a> + 'a,
    F: Fn(std::cmp::Ordering) -> bool,
{
    let left_items = left.items();
    let right_items = right.items();

    for l_item in left_items {
        for r_item in right_items {
            if let Some(ord) = compare_items_with_nodes(l_item, r_item)
                && predicate(ord)
            {
                return Ok(XdmValue::from_bool(true));
            }
        }
    }

    Ok(XdmValue::from_bool(false))
}

fn compare_items<N>(left: &XdmItem<N>, right: &XdmItem<N>) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (XdmItem::Atomic(a), XdmItem::Atomic(b)) => compare_atomics(a, b),
        _ => None,
    }
}

fn compare_items_with_nodes<'a, N>(
    left: &XdmItem<N>,
    right: &XdmItem<N>,
) -> Option<std::cmp::Ordering>
where
    N: petty_xpath1::DataSourceNode<'a> + 'a,
{
    let left_atomic = atomize_item(left);
    let right_atomic = atomize_item(right);

    match (left_atomic, right_atomic) {
        (Some(a), Some(b)) => compare_atomics(&a, &b),
        _ => None,
    }
}

fn atomize_item<'a, N>(item: &XdmItem<N>) -> Option<AtomicValue>
where
    N: petty_xpath1::DataSourceNode<'a> + 'a,
{
    match item {
        XdmItem::Atomic(a) => Some(a.clone()),
        XdmItem::Node(node) => Some(AtomicValue::UntypedAtomic(node.string_value())),
        XdmItem::Array(_) | XdmItem::Map(_) | XdmItem::Function(_) => None,
    }
}

fn compare_atomics(left: &AtomicValue, right: &AtomicValue) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (AtomicValue::String(a), AtomicValue::String(b)) => Some(a.cmp(b)),
        (AtomicValue::Integer(a), AtomicValue::Integer(b)) => Some(a.cmp(b)),
        (AtomicValue::Double(a), AtomicValue::Double(b)) => a.partial_cmp(b),
        (AtomicValue::Boolean(a), AtomicValue::Boolean(b)) => Some(a.cmp(b)),

        (AtomicValue::Integer(a), AtomicValue::Double(b)) => (*a as f64).partial_cmp(b),
        (AtomicValue::Double(a), AtomicValue::Integer(b)) => a.partial_cmp(&(*b as f64)),

        (AtomicValue::UntypedAtomic(a), AtomicValue::String(b))
        | (AtomicValue::String(b), AtomicValue::UntypedAtomic(a)) => Some(a.cmp(b)),

        (AtomicValue::UntypedAtomic(a), AtomicValue::Integer(b)) => {
            a.parse::<i64>().ok().map(|ai| ai.cmp(b))
        }
        (AtomicValue::Integer(a), AtomicValue::UntypedAtomic(b)) => {
            b.parse::<i64>().ok().map(|bi| a.cmp(&bi))
        }

        (AtomicValue::UntypedAtomic(a), AtomicValue::Double(b)) => {
            a.parse::<f64>().ok().and_then(|ad| ad.partial_cmp(b))
        }
        (AtomicValue::Double(a), AtomicValue::UntypedAtomic(b)) => {
            b.parse::<f64>().ok().and_then(|bd| a.partial_cmp(&bd))
        }

        _ => None,
    }
}

fn evaluate_arithmetic<N: Clone, F>(
    left: XdmValue<N>,
    right: XdmValue<N>,
    op: F,
) -> Result<XdmValue<N>, XPath31Error>
where
    F: Fn(f64, f64) -> f64,
{
    let l = left.to_double();
    let r = right.to_double();
    Ok(XdmValue::from_double(op(l, r)))
}

fn evaluate_divide<N: Clone>(
    left: XdmValue<N>,
    right: XdmValue<N>,
) -> Result<XdmValue<N>, XPath31Error> {
    let l = left.to_double();
    let r = right.to_double();

    if r == 0.0 {
        if l == 0.0 {
            Ok(XdmValue::from_double(f64::NAN))
        } else if l > 0.0 {
            Ok(XdmValue::from_double(f64::INFINITY))
        } else {
            Ok(XdmValue::from_double(f64::NEG_INFINITY))
        }
    } else {
        Ok(XdmValue::from_double(l / r))
    }
}

fn evaluate_modulo<N: Clone>(
    left: XdmValue<N>,
    right: XdmValue<N>,
) -> Result<XdmValue<N>, XPath31Error> {
    let l = left.to_double();
    let r = right.to_double();

    if r == 0.0 {
        Ok(XdmValue::from_double(f64::NAN))
    } else {
        Ok(XdmValue::from_double(l % r))
    }
}

fn evaluate_union<N: Clone + Eq + std::hash::Hash + Ord>(
    left: XdmValue<N>,
    right: XdmValue<N>,
) -> Result<XdmValue<N>, XPath31Error> {
    let mut nodes: Vec<N> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for item in left.items().iter().chain(right.items().iter()) {
        if let XdmItem::Node(n) = item
            && seen.insert(n.clone())
        {
            nodes.push(n.clone());
        }
    }

    nodes.sort();
    Ok(XdmValue::from_nodes(nodes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_or() {
        let t: XdmValue<()> = XdmValue::from_bool(true);
        let f: XdmValue<()> = XdmValue::from_bool(false);

        assert!(
            evaluate_binary(BinaryOperator::Or, t.clone(), f.clone())
                .unwrap()
                .effective_boolean_value()
        );
        assert!(
            evaluate_binary(BinaryOperator::Or, f.clone(), t.clone())
                .unwrap()
                .effective_boolean_value()
        );
        assert!(
            !evaluate_binary(BinaryOperator::Or, f.clone(), f.clone())
                .unwrap()
                .effective_boolean_value()
        );
    }

    #[test]
    fn test_and() {
        let t: XdmValue<()> = XdmValue::from_bool(true);
        let f: XdmValue<()> = XdmValue::from_bool(false);

        assert!(
            evaluate_binary(BinaryOperator::And, t.clone(), t.clone())
                .unwrap()
                .effective_boolean_value()
        );
        assert!(
            !evaluate_binary(BinaryOperator::And, t.clone(), f.clone())
                .unwrap()
                .effective_boolean_value()
        );
    }

    #[test]
    fn test_comparison() {
        let five: XdmValue<()> = XdmValue::from_integer(5);
        let ten: XdmValue<()> = XdmValue::from_integer(10);

        assert!(
            evaluate_binary(BinaryOperator::LessThan, five.clone(), ten.clone())
                .unwrap()
                .effective_boolean_value()
        );
        assert!(
            !evaluate_binary(BinaryOperator::LessThan, ten.clone(), five.clone())
                .unwrap()
                .effective_boolean_value()
        );
        assert!(
            evaluate_binary(BinaryOperator::Equals, five.clone(), five.clone())
                .unwrap()
                .effective_boolean_value()
        );
    }

    #[test]
    fn test_arithmetic() {
        let a: XdmValue<()> = XdmValue::from_integer(10);
        let b: XdmValue<()> = XdmValue::from_integer(3);

        let result = evaluate_binary(BinaryOperator::Plus, a.clone(), b.clone()).unwrap();
        assert_eq!(result.to_double(), 13.0);

        let result = evaluate_binary(BinaryOperator::Minus, a.clone(), b.clone()).unwrap();
        assert_eq!(result.to_double(), 7.0);

        let result = evaluate_binary(BinaryOperator::Multiply, a.clone(), b.clone()).unwrap();
        assert_eq!(result.to_double(), 30.0);
    }

    #[test]
    fn test_divide() {
        let a: XdmValue<()> = XdmValue::from_integer(10);
        let b: XdmValue<()> = XdmValue::from_integer(4);

        let result = evaluate_binary(BinaryOperator::Divide, a, b).unwrap();
        assert_eq!(result.to_double(), 2.5);
    }

    #[test]
    fn test_divide_by_zero() {
        let a: XdmValue<()> = XdmValue::from_integer(10);
        let zero: XdmValue<()> = XdmValue::from_integer(0);

        let result = evaluate_binary(BinaryOperator::Divide, a, zero).unwrap();
        assert!(result.to_double().is_infinite());
    }

    #[test]
    fn test_modulo() {
        let a: XdmValue<()> = XdmValue::from_integer(10);
        let b: XdmValue<()> = XdmValue::from_integer(3);

        let result = evaluate_binary(BinaryOperator::Modulo, a, b).unwrap();
        assert_eq!(result.to_double(), 1.0);
    }
}
