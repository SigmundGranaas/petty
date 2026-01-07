use crate::error::XPath31Error;
use crate::types::*;
use petty_xpath1::DataSourceNode;
use std::collections::HashSet;

pub fn fn_count<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("count", "Expected 1 argument"));
    }
    let seq = args.remove(0);
    Ok(XdmValue::from_integer(seq.len() as i64))
}

pub fn fn_empty<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("empty", "Expected 1 argument"));
    }
    let seq = args.remove(0);
    Ok(XdmValue::from_bool(seq.is_empty()))
}

pub fn fn_exists<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("exists", "Expected 1 argument"));
    }
    let seq = args.remove(0);
    Ok(XdmValue::from_bool(!seq.is_empty()))
}

pub fn fn_head<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("head", "Expected 1 argument"));
    }
    let seq = args.remove(0);
    match seq.first() {
        Some(item) => Ok(XdmValue::from_item(item.clone())),
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_tail<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("tail", "Expected 1 argument"));
    }
    let seq = args.remove(0);
    let items = seq.into_items();
    if items.is_empty() {
        Ok(XdmValue::empty())
    } else {
        Ok(XdmValue::from_items(items.into_iter().skip(1).collect()))
    }
}

pub fn fn_reverse<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("reverse", "Expected 1 argument"));
    }
    let seq = args.remove(0);
    let mut items = seq.into_items();
    items.reverse();
    Ok(XdmValue::from_items(items))
}

pub fn fn_subsequence<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "subsequence",
            "Expected 2 or 3 arguments",
        ));
    }

    let length = if args.len() == 3 {
        Some(args.remove(2).to_double())
    } else {
        None
    };
    let start = args.remove(1).to_double();
    let seq = args.remove(0);

    let start_idx = (start.round() as i64 - 1).max(0) as usize;
    let items = seq.items();

    let result: Vec<XdmItem<N>> = if let Some(len) = length {
        let end_idx = start_idx + len.round() as usize;
        items
            .iter()
            .skip(start_idx)
            .take(end_idx - start_idx)
            .cloned()
            .collect()
    } else {
        items.iter().skip(start_idx).cloned().collect()
    };

    Ok(XdmValue::from_items(result))
}

pub fn fn_distinct_values<N: Clone + std::hash::Hash + Eq>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "distinct-values",
            "Expected 1 or 2 arguments",
        ));
    }

    let seq = args.remove(0);
    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for item in seq.items() {
        let key = match item {
            XdmItem::Atomic(a) => a.to_string_value(),
            _ => continue,
        };

        if seen.insert(key) {
            result.push(item.clone());
        }
    }

    Ok(XdmValue::from_items(result))
}

pub fn fn_insert_before<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function(
            "insert-before",
            "Expected 3 arguments",
        ));
    }

    let inserts = args.remove(2);
    let position = args.remove(1).to_double() as usize;
    let target = args.remove(0);

    let mut items = target.into_items();
    let insert_items = inserts.into_items();

    let insert_pos = (position.saturating_sub(1)).min(items.len());
    for (i, item) in insert_items.into_iter().enumerate() {
        items.insert(insert_pos + i, item);
    }

    Ok(XdmValue::from_items(items))
}

pub fn fn_remove<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("remove", "Expected 2 arguments"));
    }

    let position = args.remove(1).to_double() as usize;
    let target = args.remove(0);

    let items: Vec<XdmItem<N>> = target
        .into_items()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i + 1 != position)
        .map(|(_, item)| item)
        .collect();

    Ok(XdmValue::from_items(items))
}

pub fn fn_deep_equal<N: Clone + PartialEq>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "deep-equal",
            "Expected 2 or 3 arguments",
        ));
    }

    let seq2 = args.remove(1);
    let seq1 = args.remove(0);

    let result = deep_equal_sequences(&seq1, &seq2);
    Ok(XdmValue::from_bool(result))
}

fn deep_equal_sequences<N: Clone + PartialEq>(seq1: &XdmValue<N>, seq2: &XdmValue<N>) -> bool {
    let items1 = seq1.items();
    let items2 = seq2.items();

    if items1.len() != items2.len() {
        return false;
    }

    for (i1, i2) in items1.iter().zip(items2.iter()) {
        if !deep_equal_items(i1, i2) {
            return false;
        }
    }

    true
}

fn deep_equal_items<N: Clone + PartialEq>(item1: &XdmItem<N>, item2: &XdmItem<N>) -> bool {
    match (item1, item2) {
        (XdmItem::Atomic(a1), XdmItem::Atomic(a2)) => deep_equal_atomics(a1, a2),
        (XdmItem::Node(n1), XdmItem::Node(n2)) => n1 == n2,
        (XdmItem::Map(m1), XdmItem::Map(m2)) => deep_equal_maps(m1, m2),
        (XdmItem::Array(a1), XdmItem::Array(a2)) => deep_equal_arrays(a1, a2),
        (XdmItem::Function(_), XdmItem::Function(_)) => false,
        _ => false,
    }
}

fn deep_equal_atomics(a1: &AtomicValue, a2: &AtomicValue) -> bool {
    match (a1, a2) {
        (AtomicValue::Boolean(b1), AtomicValue::Boolean(b2)) => b1 == b2,
        (AtomicValue::Integer(i1), AtomicValue::Integer(i2)) => i1 == i2,
        (AtomicValue::Double(d1), AtomicValue::Double(d2)) => {
            if d1.is_nan() && d2.is_nan() {
                true
            } else {
                (d1 - d2).abs() < f64::EPSILON
            }
        }
        (AtomicValue::Decimal(d1), AtomicValue::Decimal(d2)) => d1 == d2,
        (AtomicValue::String(s1), AtomicValue::String(s2)) => s1 == s2,
        (AtomicValue::Integer(i), AtomicValue::Double(d))
        | (AtomicValue::Double(d), AtomicValue::Integer(i)) => {
            (*i as f64 - *d).abs() < f64::EPSILON
        }
        (AtomicValue::DateTime(d1), AtomicValue::DateTime(d2)) => d1 == d2,
        (AtomicValue::Date(d1), AtomicValue::Date(d2)) => d1 == d2,
        (AtomicValue::Time(t1), AtomicValue::Time(t2)) => t1 == t2,
        (AtomicValue::Duration(d1), AtomicValue::Duration(d2)) => d1 == d2,
        (AtomicValue::QName { .. }, AtomicValue::QName { .. }) => a1 == a2,
        _ => false,
    }
}

fn deep_equal_maps<N: Clone + PartialEq>(m1: &XdmMap<N>, m2: &XdmMap<N>) -> bool {
    if m1.size() != m2.size() {
        return false;
    }

    for (key, val1) in m1.entries() {
        match m2.get(key) {
            Some(val2) => {
                if !deep_equal_sequences(val1, val2) {
                    return false;
                }
            }
            None => return false,
        }
    }

    true
}

fn deep_equal_arrays<N: Clone + PartialEq>(a1: &XdmArray<N>, a2: &XdmArray<N>) -> bool {
    if a1.size() != a2.size() {
        return false;
    }

    for i in 1..=a1.size() {
        match (a1.get(i), a2.get(i)) {
            (Some(v1), Some(v2)) => {
                if !deep_equal_sequences(v1, v2) {
                    return false;
                }
            }
            _ => return false,
        }
    }

    true
}

pub fn fn_index_of<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "index-of",
            "Expected 2 or 3 arguments",
        ));
    }

    let search = args.remove(1);
    let seq = args.remove(0);

    let search_val = match search.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => return Ok(XdmValue::empty()),
    };

    let indices: Vec<XdmItem<N>> = seq
        .items()
        .iter()
        .enumerate()
        .filter_map(|(i, item)| match item {
            XdmItem::Atomic(a) if deep_equal_atomics(a, &search_val) => {
                Some(XdmItem::Atomic(AtomicValue::Integer((i + 1) as i64)))
            }
            _ => None,
        })
        .collect();

    Ok(XdmValue::from_items(indices))
}

pub fn fn_zero_or_one<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("zero-or-one", "Expected 1 argument"));
    }

    let seq = args.remove(0);
    if seq.len() > 1 {
        return Err(XPath31Error::cardinality_error(
            "zero-or-one",
            "expected zero or one items",
            seq.len(),
        ));
    }

    Ok(seq)
}

pub fn fn_one_or_more<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("one-or-more", "Expected 1 argument"));
    }

    let seq = args.remove(0);
    if seq.is_empty() {
        return Err(XPath31Error::cardinality_error(
            "one-or-more",
            "expected one or more items",
            0,
        ));
    }

    Ok(seq)
}

pub fn fn_exactly_one<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("exactly-one", "Expected 1 argument"));
    }

    let seq = args.remove(0);
    if seq.len() != 1 {
        return Err(XPath31Error::cardinality_error(
            "exactly-one",
            "expected exactly one item",
            seq.len(),
        ));
    }

    Ok(seq)
}

pub fn fn_innermost<'a, N: DataSourceNode<'a> + Clone + PartialEq>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("innermost", "Expected 1 argument"));
    }

    let seq = args.remove(0);
    let nodes: Vec<N> = seq
        .items()
        .iter()
        .filter_map(|item| match item {
            XdmItem::Node(n) => Some(*n),
            _ => None,
        })
        .collect();

    let result: Vec<XdmItem<N>> = nodes
        .iter()
        .filter(|&node| {
            !nodes.iter().any(|other| {
                if node == other {
                    return false;
                }
                is_ancestor_of(*other, *node)
            })
        })
        .map(|n| XdmItem::Node(*n))
        .collect();

    Ok(XdmValue::from_items(result))
}

pub fn fn_outermost<'a, N: DataSourceNode<'a> + Clone + PartialEq>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("outermost", "Expected 1 argument"));
    }

    let seq = args.remove(0);
    let nodes: Vec<N> = seq
        .items()
        .iter()
        .filter_map(|item| match item {
            XdmItem::Node(n) => Some(*n),
            _ => None,
        })
        .collect();

    let result: Vec<XdmItem<N>> = nodes
        .iter()
        .filter(|&node| {
            !nodes.iter().any(|other| {
                if node == other {
                    return false;
                }
                is_ancestor_of(*node, *other)
            })
        })
        .map(|n| XdmItem::Node(*n))
        .collect();

    Ok(XdmValue::from_items(result))
}

fn is_ancestor_of<'a, N: DataSourceNode<'a> + Clone + PartialEq>(
    ancestor: N,
    descendant: N,
) -> bool {
    let mut current = descendant.parent();
    while let Some(parent) = current {
        if parent == ancestor {
            return true;
        }
        current = parent.parent();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int_seq<N: Clone>(values: &[i64]) -> XdmValue<N> {
        XdmValue::from_items(
            values
                .iter()
                .map(|&v| XdmItem::Atomic(AtomicValue::Integer(v)))
                .collect(),
        )
    }

    #[test]
    fn test_count() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 3, 4, 5]);
        let result = fn_count(vec![seq]).unwrap();
        assert_eq!(result.to_double(), 5.0);
    }

    #[test]
    fn test_empty_exists() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 3]);
        let empty_seq: XdmValue<()> = XdmValue::empty();

        assert!(
            !fn_empty(vec![seq.clone()])
                .unwrap()
                .effective_boolean_value()
        );
        assert!(
            fn_empty(vec![empty_seq.clone()])
                .unwrap()
                .effective_boolean_value()
        );

        assert!(fn_exists(vec![seq]).unwrap().effective_boolean_value());
        assert!(
            !fn_exists(vec![empty_seq])
                .unwrap()
                .effective_boolean_value()
        );
    }

    #[test]
    fn test_head_tail() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 3, 4, 5]);

        let head = fn_head(vec![seq.clone()]).unwrap();
        assert_eq!(head.to_double(), 1.0);

        let tail = fn_tail(vec![seq]).unwrap();
        assert_eq!(tail.len(), 4);
    }

    #[test]
    fn test_reverse() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 3]);
        let reversed = fn_reverse(vec![seq]).unwrap();
        assert_eq!(reversed.len(), 3);
    }

    #[test]
    fn test_subsequence() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 3, 4, 5]);

        let sub = fn_subsequence(vec![
            seq.clone(),
            XdmValue::from_integer(2),
            XdmValue::from_integer(3),
        ])
        .unwrap();
        assert_eq!(sub.len(), 3);

        let sub = fn_subsequence(vec![seq, XdmValue::from_integer(3)]).unwrap();
        assert_eq!(sub.len(), 3);
    }

    #[test]
    fn test_distinct_values() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 2, 3, 3, 3]);
        let distinct = fn_distinct_values(vec![seq]).unwrap();
        assert_eq!(distinct.len(), 3);
    }

    #[test]
    fn test_insert_before() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 4, 5]);
        let insert: XdmValue<()> = int_seq(&[3]);

        let result = fn_insert_before(vec![seq, XdmValue::from_integer(3), insert]).unwrap();
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_remove() {
        let seq: XdmValue<()> = int_seq(&[1, 2, 3, 4, 5]);
        let result = fn_remove(vec![seq, XdmValue::from_integer(3)]).unwrap();
        assert_eq!(result.len(), 4);
    }
}
