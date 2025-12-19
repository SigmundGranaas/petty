use thiserror::Error;
use petty_template_core::TemplateError;

#[derive(Error, Debug)]
pub enum JsonTemplateError {
    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("JPath evaluation error: {0}")]
    JPath(#[from] petty_jpath::JPathError),

    #[error("Template compilation error: {0}")]
    Compilation(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Template render error: {0}")]
    TemplateRender(String),

    #[error("Template parse error: {0}")]
    TemplateParse(String),
}

impl From<JsonTemplateError> for TemplateError {
    fn from(err: JsonTemplateError) -> Self {
        match err {
            JsonTemplateError::JsonParse(e) => TemplateError::ParseError(e.to_string()),
            JsonTemplateError::JPath(e) => TemplateError::ExecutionError(e.to_string()),
            JsonTemplateError::Compilation(s) => TemplateError::ParseError(s),
            JsonTemplateError::Execution(s) => TemplateError::ExecutionError(s),
            JsonTemplateError::TemplateRender(s) => TemplateError::ExecutionError(s),
            JsonTemplateError::TemplateParse(s) => TemplateError::ParseError(s),
        }
    }
}
