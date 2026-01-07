use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum XPath31Error {
    #[error("Parse error in '{expression}': {message}")]
    ParseError { expression: String, message: String },

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Dynamic error: {0}")]
    DynamicError(String),

    #[error("Function '{function}' error: {message}")]
    FunctionError { function: String, message: String },

    #[error("Variable '${name}' not found")]
    UnknownVariable { name: String },

    #[error("Key '{key}' not found in map")]
    KeyNotFound { key: String },

    #[error("Array index {index} out of bounds (size: {size})")]
    ArrayIndexOutOfBounds { index: i64, size: usize },

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Cannot cast {from_type} to {to_type}")]
    InvalidCast { from_type: String, to_type: String },

    #[error("Cardinality error: expected {expected}, got {actual} items")]
    CardinalityError { expected: String, actual: usize },

    #[error("Context item is required but not set")]
    NoContextItem,

    #[error("XPath 1.0 error: {0}")]
    XPath1Error(#[from] petty_xpath1::XPathError),
}

impl XPath31Error {
    pub fn parse(expression: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ParseError {
            expression: expression.into(),
            message: message.into(),
        }
    }

    pub fn function(function: impl Into<String>, message: impl Into<String>) -> Self {
        Self::FunctionError {
            function: function.into(),
            message: message.into(),
        }
    }

    pub fn type_error(message: impl Into<String>) -> Self {
        Self::TypeError(message.into())
    }

    pub fn dynamic_error(message: impl Into<String>) -> Self {
        Self::DynamicError(message.into())
    }

    pub fn cardinality_error(
        _function: impl Into<String>,
        expected: impl Into<String>,
        actual: usize,
    ) -> Self {
        Self::CardinalityError {
            expected: expected.into(),
            actual,
        }
    }
}
