use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum JPathError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("JPath parse error in '{0}': {1}")]
    JPathParse(String, String),

    #[error("Function '{function}' error: {message}")]
    FunctionError { function: String, message: String },

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Template render error: {0}")]
    TemplateRender(String),
}
