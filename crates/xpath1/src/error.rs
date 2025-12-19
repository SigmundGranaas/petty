use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum XPathError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("XPath parse error in '{0}': {1}")]
    XPathParse(String, String),

    #[error("Function '{function}' error: {message}")]
    FunctionError { function: String, message: String },

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Variable '{0}' not found")]
    UnknownVariable(String),

    #[error("Context node required")]
    NoContextNode,
}
