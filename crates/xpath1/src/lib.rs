pub mod ast;
pub mod axes;
pub mod datasource;
pub mod engine;
pub mod error;
pub mod functions;
pub mod operators;
pub mod parser;

pub use ast::{Axis, BinaryOperator, Expression, LocationPath, NodeTest, Step};
pub use datasource::{DataSourceNode, NodeType, QName};
pub use engine::{EvaluationContext, XPathValue, evaluate};

// Re-export test utilities for integration testing in downstream crates
pub use datasource::tests;
pub use error::XPathError;
pub use parser::parse_expression;
