use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::ast::{Expression, Param};

#[derive(Clone)]
pub enum XdmFunction<N> {
    Builtin {
        name: String,
        arity: usize,
    },
    Inline {
        params: Vec<Param>,
        body: Arc<Expression>,
        captured_variables: Vec<(String, super::XdmValue<N>)>,
    },
    NamedRef {
        namespace: Option<String>,
        local_name: String,
        arity: usize,
    },
    Partial {
        base: Box<XdmFunction<N>>,
        bound_args: Vec<Option<super::XdmValue<N>>>,
    },
}

impl<N> XdmFunction<N> {
    pub fn builtin(name: impl Into<String>, arity: usize) -> Self {
        Self::Builtin {
            name: name.into(),
            arity,
        }
    }

    pub fn named_ref(
        namespace: Option<String>,
        local_name: impl Into<String>,
        arity: usize,
    ) -> Self {
        Self::NamedRef {
            namespace,
            local_name: local_name.into(),
            arity,
        }
    }

    pub fn inline(
        params: Vec<Param>,
        body: Expression,
        captured: Vec<(String, super::XdmValue<N>)>,
    ) -> Self {
        Self::Inline {
            params,
            body: Arc::new(body),
            captured_variables: captured,
        }
    }

    pub fn arity(&self) -> usize {
        match self {
            XdmFunction::Builtin { arity, .. } => *arity,
            XdmFunction::Inline { params, .. } => params.len(),
            XdmFunction::NamedRef { arity, .. } => *arity,
            XdmFunction::Partial { base, bound_args } => {
                base.arity() - bound_args.iter().filter(|a| a.is_some()).count()
            }
        }
    }

    pub fn name(&self) -> Option<String> {
        match self {
            XdmFunction::Builtin { name, .. } => Some(name.clone()),
            XdmFunction::NamedRef {
                namespace,
                local_name,
                ..
            } => match namespace {
                Some(ns) => Some(format!("{}:{}", ns, local_name)),
                None => Some(local_name.clone()),
            },
            XdmFunction::Inline { .. } => None,
            XdmFunction::Partial { base, .. } => base.name(),
        }
    }
}

impl<N> fmt::Debug for XdmFunction<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XdmFunction::Builtin { name, arity } => {
                write!(f, "function {}#{}", name, arity)
            }
            XdmFunction::Inline { params, .. } => {
                write!(
                    f,
                    "function(${})",
                    params
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", $")
                )
            }
            XdmFunction::NamedRef {
                namespace,
                local_name,
                arity,
            } => match namespace {
                Some(ns) => write!(f, "{}:{}#{}", ns, local_name, arity),
                None => write!(f, "{}#{}", local_name, arity),
            },
            XdmFunction::Partial { base, .. } => {
                write!(f, "partial({:?})", base)
            }
        }
    }
}

impl<N> fmt::Display for XdmFunction<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<N> PartialEq for XdmFunction<N> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                XdmFunction::Builtin {
                    name: n1,
                    arity: a1,
                },
                XdmFunction::Builtin {
                    name: n2,
                    arity: a2,
                },
            ) => n1 == n2 && a1 == a2,
            (
                XdmFunction::NamedRef {
                    namespace: ns1,
                    local_name: ln1,
                    arity: a1,
                },
                XdmFunction::NamedRef {
                    namespace: ns2,
                    local_name: ln2,
                    arity: a2,
                },
            ) => ns1 == ns2 && ln1 == ln2 && a1 == a2,
            _ => false,
        }
    }
}

impl<N> Eq for XdmFunction<N> {}

impl<N> Hash for XdmFunction<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            XdmFunction::Builtin { name, arity } => {
                name.hash(state);
                arity.hash(state);
            }
            XdmFunction::NamedRef {
                namespace,
                local_name,
                arity,
            } => {
                namespace.hash(state);
                local_name.hash(state);
                arity.hash(state);
            }
            XdmFunction::Inline { params, .. } => {
                params.len().hash(state);
            }
            XdmFunction::Partial { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_function() {
        let f: XdmFunction<()> = XdmFunction::builtin("concat", 3);
        assert_eq!(f.arity(), 3);
        assert_eq!(f.name(), Some("concat".to_string()));
    }

    #[test]
    fn test_named_ref() {
        let f: XdmFunction<()> = XdmFunction::named_ref(Some("fn".to_string()), "concat", 3);
        assert_eq!(f.arity(), 3);
        assert_eq!(f.name(), Some("fn:concat".to_string()));
    }
}
