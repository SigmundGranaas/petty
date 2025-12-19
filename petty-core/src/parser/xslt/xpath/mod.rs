//! A generic, XPath 1.0 engine that operates over any data source conforming
//! to the `DataSourceNode` trait.

pub mod ast;
pub mod engine;
pub mod functions;
pub mod parser;
pub mod axes;
pub mod operators;

// --- Public API ---
pub use self::ast::Expression;
pub use self::engine::{evaluate, EvaluationContext, XPathValue};
pub use self::functions::FunctionRegistry;
pub use self::parser::parse_expression;