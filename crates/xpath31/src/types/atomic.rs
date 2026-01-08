use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub enum AtomicValue {
    String(String),
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    Double(f64),
    Date(String),
    DateTime(String),
    Time(String),
    Duration(String),
    QName {
        prefix: Option<String>,
        local: String,
        namespace: Option<String>,
    },
    UntypedAtomic(String),
}

impl AtomicValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            AtomicValue::String(_) => "xs:string",
            AtomicValue::Boolean(_) => "xs:boolean",
            AtomicValue::Integer(_) => "xs:integer",
            AtomicValue::Decimal(_) => "xs:decimal",
            AtomicValue::Double(_) => "xs:double",
            AtomicValue::Date(_) => "xs:date",
            AtomicValue::DateTime(_) => "xs:dateTime",
            AtomicValue::Time(_) => "xs:time",
            AtomicValue::Duration(_) => "xs:duration",
            AtomicValue::QName { .. } => "xs:QName",
            AtomicValue::UntypedAtomic(_) => "xs:untypedAtomic",
        }
    }

    pub fn to_string_value(&self) -> String {
        match self {
            AtomicValue::String(s) | AtomicValue::UntypedAtomic(s) => s.clone(),
            AtomicValue::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
            AtomicValue::Integer(i) => i.to_string(),
            AtomicValue::Decimal(d) => d.to_string(),
            AtomicValue::Double(d) => {
                if d.is_nan() {
                    "NaN".to_string()
                } else if d.is_infinite() {
                    if *d > 0.0 { "INF" } else { "-INF" }.to_string()
                } else if *d == 0.0 && d.is_sign_negative() {
                    "-0".to_string()
                } else {
                    d.to_string()
                }
            }
            AtomicValue::Date(s)
            | AtomicValue::DateTime(s)
            | AtomicValue::Time(s)
            | AtomicValue::Duration(s) => s.clone(),
            AtomicValue::QName { prefix, local, .. } => match prefix {
                Some(p) => format!("{}:{}", p, local),
                None => local.clone(),
            },
        }
    }

    pub fn to_boolean(&self) -> bool {
        match self {
            AtomicValue::Boolean(b) => *b,
            AtomicValue::String(s) | AtomicValue::UntypedAtomic(s) => !s.is_empty(),
            AtomicValue::Integer(i) => *i != 0,
            AtomicValue::Decimal(d) => !d.is_zero(),
            AtomicValue::Double(d) => *d != 0.0 && !d.is_nan(),
            _ => true,
        }
    }

    pub fn to_double(&self) -> f64 {
        match self {
            AtomicValue::Double(d) => *d,
            AtomicValue::Integer(i) => *i as f64,
            AtomicValue::Decimal(d) => d.to_string().parse().unwrap_or(f64::NAN),
            AtomicValue::String(s) | AtomicValue::UntypedAtomic(s) => {
                s.trim().parse().unwrap_or(f64::NAN)
            }
            AtomicValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => f64::NAN,
        }
    }

    pub fn to_integer(&self) -> Option<i64> {
        match self {
            AtomicValue::Integer(i) => Some(*i),
            AtomicValue::Double(d) => {
                if d.is_finite() {
                    Some(d.trunc() as i64)
                } else {
                    None
                }
            }
            AtomicValue::Decimal(d) => d.to_string().parse().ok(),
            AtomicValue::String(s) | AtomicValue::UntypedAtomic(s) => s.trim().parse().ok(),
            AtomicValue::Boolean(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            AtomicValue::Integer(_) | AtomicValue::Decimal(_) | AtomicValue::Double(_)
        )
    }
}

impl PartialEq for AtomicValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AtomicValue::String(a), AtomicValue::String(b)) => a == b,
            (AtomicValue::Boolean(a), AtomicValue::Boolean(b)) => a == b,
            (AtomicValue::Integer(a), AtomicValue::Integer(b)) => a == b,
            (AtomicValue::Integer(a), AtomicValue::Double(b))
            | (AtomicValue::Double(b), AtomicValue::Integer(a)) => (*a as f64) == *b,
            (AtomicValue::Double(a), AtomicValue::Double(b)) => {
                a == b || (a.is_nan() && b.is_nan())
            }
            (AtomicValue::Decimal(a), AtomicValue::Decimal(b)) => a == b,
            (AtomicValue::UntypedAtomic(a), AtomicValue::UntypedAtomic(b)) => a == b,
            (AtomicValue::UntypedAtomic(a), AtomicValue::String(b))
            | (AtomicValue::String(b), AtomicValue::UntypedAtomic(a)) => a == b,
            _ => false,
        }
    }
}

impl Eq for AtomicValue {}

impl Hash for AtomicValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            AtomicValue::String(s) | AtomicValue::UntypedAtomic(s) => s.hash(state),
            AtomicValue::Boolean(b) => b.hash(state),
            AtomicValue::Integer(i) => i.hash(state),
            AtomicValue::Decimal(d) => d.to_string().hash(state),
            AtomicValue::Double(d) => d.to_bits().hash(state),
            AtomicValue::Date(s)
            | AtomicValue::DateTime(s)
            | AtomicValue::Time(s)
            | AtomicValue::Duration(s) => s.hash(state),
            AtomicValue::QName {
                prefix,
                local,
                namespace,
            } => {
                prefix.hash(state);
                local.hash(state);
                namespace.hash(state);
            }
        }
    }
}

impl PartialOrd for AtomicValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (AtomicValue::String(a), AtomicValue::String(b)) => a.partial_cmp(b),
            (AtomicValue::Integer(a), AtomicValue::Integer(b)) => a.partial_cmp(b),
            (AtomicValue::Double(a), AtomicValue::Double(b)) => a.partial_cmp(b),
            (AtomicValue::Decimal(a), AtomicValue::Decimal(b)) => a.partial_cmp(b),
            (AtomicValue::Integer(a), AtomicValue::Double(b)) => (*a as f64).partial_cmp(b),
            (AtomicValue::Double(a), AtomicValue::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (AtomicValue::Boolean(a), AtomicValue::Boolean(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

impl fmt::Display for AtomicValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_value())
    }
}

impl From<String> for AtomicValue {
    fn from(s: String) -> Self {
        AtomicValue::String(s)
    }
}

impl From<&str> for AtomicValue {
    fn from(s: &str) -> Self {
        AtomicValue::String(s.to_string())
    }
}

impl From<bool> for AtomicValue {
    fn from(b: bool) -> Self {
        AtomicValue::Boolean(b)
    }
}

impl From<i64> for AtomicValue {
    fn from(i: i64) -> Self {
        AtomicValue::Integer(i)
    }
}

impl From<i32> for AtomicValue {
    fn from(i: i32) -> Self {
        AtomicValue::Integer(i as i64)
    }
}

impl From<f64> for AtomicValue {
    fn from(d: f64) -> Self {
        AtomicValue::Double(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_operations() {
        let s = AtomicValue::String("hello".to_string());
        assert_eq!(s.to_string_value(), "hello");
        assert!(s.to_boolean());
        assert!(AtomicValue::String("".to_string()).to_boolean() == false);
    }

    #[test]
    fn test_boolean_operations() {
        assert_eq!(AtomicValue::Boolean(true).to_string_value(), "true");
        assert_eq!(AtomicValue::Boolean(false).to_string_value(), "false");
        assert_eq!(AtomicValue::Boolean(true).to_double(), 1.0);
        assert_eq!(AtomicValue::Boolean(false).to_double(), 0.0);
    }

    #[test]
    fn test_numeric_operations() {
        let int = AtomicValue::Integer(42);
        assert_eq!(int.to_double(), 42.0);
        assert_eq!(int.to_integer(), Some(42));
        assert!(int.is_numeric());

        let dbl = AtomicValue::Double(3.56);
        assert_eq!(dbl.to_integer(), Some(3));
        assert!(dbl.is_numeric());
    }

    #[test]
    fn test_equality() {
        assert_eq!(AtomicValue::Integer(5), AtomicValue::Integer(5));
        assert_eq!(AtomicValue::Integer(5), AtomicValue::Double(5.0));
        assert_ne!(AtomicValue::Integer(5), AtomicValue::Integer(6));
    }

    #[test]
    fn test_comparison() {
        assert!(AtomicValue::Integer(5) < AtomicValue::Integer(10));
        assert!(AtomicValue::Double(3.56) < AtomicValue::Double(4.0));
        assert!(AtomicValue::String("abc".to_string()) < AtomicValue::String("def".to_string()));
    }
}
