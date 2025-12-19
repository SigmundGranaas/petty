pub mod ast;
pub mod parser;
pub mod engine;
pub mod functions;
pub mod operators;
pub mod axes;
pub mod datasource;
pub mod error;

pub use ast::{Expression, LocationPath, Step, Axis, NodeTest, BinaryOperator};
pub use engine::{XPathValue, EvaluationContext, evaluate};
pub use datasource::{DataSourceNode, NodeType, QName};

// Re-export test utilities for integration testing in downstream crates
pub use datasource::tests;
pub use error::XPathError;
pub use parser::parse_expression;
