use super::{AtomicValue, XdmArray, XdmFunction, XdmMap};
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub enum XdmItem<N> {
    Node(N),
    Atomic(AtomicValue),
    Map(XdmMap<N>),
    Array(XdmArray<N>),
    Function(XdmFunction<N>),
}

impl<N: Clone> XdmItem<N> {
    pub fn is_node(&self) -> bool {
        matches!(self, XdmItem::Node(_))
    }

    pub fn is_atomic(&self) -> bool {
        matches!(self, XdmItem::Atomic(_))
    }

    pub fn is_map(&self) -> bool {
        matches!(self, XdmItem::Map(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, XdmItem::Array(_))
    }

    pub fn is_function(&self) -> bool {
        matches!(self, XdmItem::Function(_))
    }

    pub fn as_node(&self) -> Option<&N> {
        match self {
            XdmItem::Node(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_atomic(&self) -> Option<&AtomicValue> {
        match self {
            XdmItem::Atomic(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&XdmMap<N>> {
        match self {
            XdmItem::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&XdmArray<N>> {
        match self {
            XdmItem::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_function(&self) -> Option<&XdmFunction<N>> {
        match self {
            XdmItem::Function(f) => Some(f),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            XdmItem::Node(_) => "node()",
            XdmItem::Atomic(a) => a.type_name(),
            XdmItem::Map(_) => "map(*)",
            XdmItem::Array(_) => "array(*)",
            XdmItem::Function(_) => "function(*)",
        }
    }

    pub fn string_value(&self) -> String
    where
        N: petty_xpath1::DataSourceNode<'static>,
    {
        match self {
            XdmItem::Node(n) => n.string_value(),
            XdmItem::Atomic(a) => a.to_string_value(),
            XdmItem::Map(m) => m.to_string(),
            XdmItem::Array(a) => a.to_string(),
            XdmItem::Function(f) => f.to_string(),
        }
    }
}

impl<N: PartialEq + Clone> PartialEq for XdmItem<N> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (XdmItem::Node(a), XdmItem::Node(b)) => a == b,
            (XdmItem::Atomic(a), XdmItem::Atomic(b)) => a == b,
            (XdmItem::Map(a), XdmItem::Map(b)) => a == b,
            (XdmItem::Array(a), XdmItem::Array(b)) => a == b,
            (XdmItem::Function(a), XdmItem::Function(b)) => a == b,
            _ => false,
        }
    }
}

impl<N: Eq + Clone> Eq for XdmItem<N> {}

impl<N: Hash + Clone> Hash for XdmItem<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            XdmItem::Node(n) => n.hash(state),
            XdmItem::Atomic(a) => a.hash(state),
            XdmItem::Map(m) => m.hash(state),
            XdmItem::Array(a) => a.hash(state),
            XdmItem::Function(f) => f.hash(state),
        }
    }
}

#[derive(Debug, Clone)]
pub enum XdmValue<N> {
    Sequence(Vec<XdmItem<N>>),
}

impl<N: Clone> XdmValue<N> {
    pub fn empty() -> Self {
        Self::Sequence(vec![])
    }

    pub fn from_item(item: XdmItem<N>) -> Self {
        Self::Sequence(vec![item])
    }

    pub fn from_items(items: Vec<XdmItem<N>>) -> Self {
        Self::Sequence(items)
    }

    pub fn from_atomic(value: AtomicValue) -> Self {
        Self::from_item(XdmItem::Atomic(value))
    }

    pub fn from_node(node: N) -> Self {
        Self::from_item(XdmItem::Node(node))
    }

    pub fn from_nodes(nodes: Vec<N>) -> Self {
        Self::from_items(nodes.into_iter().map(XdmItem::Node).collect())
    }

    pub fn from_map(map: XdmMap<N>) -> Self {
        Self::from_item(XdmItem::Map(map))
    }

    pub fn from_array(array: XdmArray<N>) -> Self {
        Self::from_item(XdmItem::Array(array))
    }

    pub fn from_function(func: XdmFunction<N>) -> Self {
        Self::from_item(XdmItem::Function(func))
    }

    pub fn from_bool(b: bool) -> Self {
        Self::from_atomic(AtomicValue::Boolean(b))
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self::from_atomic(AtomicValue::String(s.into()))
    }

    pub fn from_integer(i: i64) -> Self {
        Self::from_atomic(AtomicValue::Integer(i))
    }

    pub fn from_double(d: f64) -> Self {
        Self::from_atomic(AtomicValue::Double(d))
    }

    pub fn is_empty(&self) -> bool {
        match self {
            XdmValue::Sequence(items) => items.is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            XdmValue::Sequence(items) => items.len(),
        }
    }

    pub fn items(&self) -> &[XdmItem<N>] {
        match self {
            XdmValue::Sequence(items) => items,
        }
    }

    pub fn into_items(self) -> Vec<XdmItem<N>> {
        match self {
            XdmValue::Sequence(items) => items,
        }
    }

    pub fn first(&self) -> Option<&XdmItem<N>> {
        self.items().first()
    }

    pub fn single(&self) -> Option<&XdmItem<N>> {
        let items = self.items();
        if items.len() == 1 {
            items.first()
        } else {
            None
        }
    }

    pub fn concat(self, other: XdmValue<N>) -> Self {
        let mut items = self.into_items();
        items.extend(other.into_items());
        Self::from_items(items)
    }

    pub fn effective_boolean_value(&self) -> bool {
        let items = self.items();
        if items.is_empty() {
            return false;
        }
        if items.len() == 1 {
            match &items[0] {
                XdmItem::Node(_) => true,
                XdmItem::Atomic(a) => a.to_boolean(),
                XdmItem::Map(_) | XdmItem::Array(_) | XdmItem::Function(_) => true,
            }
        } else {
            true
        }
    }

    pub fn to_double(&self) -> f64 {
        match self.first() {
            Some(XdmItem::Atomic(a)) => a.to_double(),
            Some(XdmItem::Node(_)) => f64::NAN,
            _ => f64::NAN,
        }
    }

    pub fn to_string_value(&self) -> String {
        match self.first() {
            Some(XdmItem::Atomic(a)) => a.to_string_value(),
            _ => String::new(),
        }
    }
}

impl<'a, N: petty_xpath1::DataSourceNode<'a> + Clone + 'a> XdmValue<N> {
    pub fn to_xpath_string(&self) -> String {
        match self.first() {
            Some(XdmItem::Atomic(a)) => a.to_string_value(),
            Some(XdmItem::Node(node)) => node.string_value(),
            _ => String::new(),
        }
    }

    pub fn to_nodes(&self) -> Vec<N> {
        self.items()
            .iter()
            .filter_map(|item| {
                if let XdmItem::Node(n) = item {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn atomize(&self) -> Self {
        let atoms: Vec<XdmItem<N>> = self
            .items()
            .iter()
            .flat_map(|item| match item {
                XdmItem::Atomic(a) => vec![XdmItem::Atomic(a.clone())],
                XdmItem::Node(_) => vec![],
                XdmItem::Array(arr) => arr
                    .members()
                    .iter()
                    .flat_map(|m| m.atomize().into_items())
                    .collect(),
                XdmItem::Map(_) | XdmItem::Function(_) => vec![],
            })
            .collect();
        Self::from_items(atoms)
    }
}

impl<N: PartialEq + Clone> PartialEq for XdmValue<N> {
    fn eq(&self, other: &Self) -> bool {
        self.items() == other.items()
    }
}

impl<N: Eq + Clone> Eq for XdmValue<N> {}

impl<N: Hash + Clone> Hash for XdmValue<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let items = self.items();
        items.len().hash(state);
        for item in items {
            item.hash(state);
        }
    }
}

impl<N: fmt::Debug + Clone> fmt::Display for XdmValue<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let items = self.items();
        if items.is_empty() {
            write!(f, "()")
        } else if items.len() == 1 {
            write!(f, "{:?}", items[0])
        } else {
            write!(f, "(")?;
            let mut first = true;
            for item in items {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{:?}", item)?;
                first = false;
            }
            write!(f, ")")
        }
    }
}

impl<N: Clone> From<AtomicValue> for XdmValue<N> {
    fn from(v: AtomicValue) -> Self {
        Self::from_atomic(v)
    }
}

impl<N: Clone> From<bool> for XdmValue<N> {
    fn from(b: bool) -> Self {
        Self::from_bool(b)
    }
}

impl<N: Clone> From<i64> for XdmValue<N> {
    fn from(i: i64) -> Self {
        Self::from_integer(i)
    }
}

impl<N: Clone> From<f64> for XdmValue<N> {
    fn from(d: f64) -> Self {
        Self::from_double(d)
    }
}

impl<N: Clone> From<String> for XdmValue<N> {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl<N: Clone> From<&str> for XdmValue<N> {
    fn from(s: &str) -> Self {
        Self::from_string(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_sequence() {
        let v: XdmValue<()> = XdmValue::empty();
        assert!(v.is_empty());
        assert_eq!(v.len(), 0);
        assert!(!v.effective_boolean_value());
    }

    #[test]
    fn test_single_value() {
        let v: XdmValue<()> = XdmValue::from_integer(42);
        assert!(!v.is_empty());
        assert_eq!(v.len(), 1);
        assert!(v.single().is_some());
    }

    #[test]
    fn test_effective_boolean_value() {
        assert!(XdmValue::<()>::from_bool(true).effective_boolean_value());
        assert!(!XdmValue::<()>::from_bool(false).effective_boolean_value());
        assert!(!XdmValue::<()>::from_string("").effective_boolean_value());
        assert!(XdmValue::<()>::from_string("x").effective_boolean_value());
        assert!(!XdmValue::<()>::from_integer(0).effective_boolean_value());
        assert!(XdmValue::<()>::from_integer(1).effective_boolean_value());
    }

    #[test]
    fn test_concat() {
        let v1: XdmValue<()> = XdmValue::from_integer(1);
        let v2: XdmValue<()> = XdmValue::from_integer(2);
        let combined = v1.concat(v2);
        assert_eq!(combined.len(), 2);
    }
}
