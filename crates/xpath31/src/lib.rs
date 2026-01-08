//! XPath 3.1 expression parser and evaluator.
//!
//! Extends XPath 1.0 with 3.1 features: maps, arrays, higher-order functions,
//! `let`/`for`/`if` expressions, arrow operators, and the full XDM type system.
//!
//! # Key Types
//!
//! - [`Expression`]: Parsed XPath 3.1 expression AST
//! - [`XdmValue`]: XDM sequence value (nodes, atomics, maps, arrays, functions)
//! - [`EvaluationContext`]: Context for expression evaluation
//!
//! # Example
//!
//! ```ignore
//! use petty_xpath31::{parse_expression, evaluate, EvaluationContext, XdmValue};
//!
//! let expr = parse_expression("for $i in 1 to 5 return $i * 2")?;
//! let ctx = EvaluationContext::new(None, None, &variables);
//! let result: XdmValue<_> = evaluate(&expr, &ctx, &HashMap::new())?;
//! ```

pub mod ast;
pub mod engine;
pub mod error;
pub mod functions;
pub mod operators;
pub mod parser;
pub mod types;

pub use ast::{ArrayConstructorKind, Expression, LookupKey, Param, Quantifier, SequenceType};
pub use engine::{EvaluationContext, evaluate};
pub use error::XPath31Error;
pub use parser::parse_expression;
pub use types::{AtomicValue, XdmArray, XdmFunction, XdmItem, XdmMap, XdmValue};

pub use petty_xpath1::{DataSourceNode, NodeType, QName};
