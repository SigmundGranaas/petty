use petty_template_core::TemplateError;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, column {}", self.line, self.col)
    }
}

impl From<(usize, usize)> for Location {
    fn from((line, col): (usize, usize)) -> Self {
        Location { line, col }
    }
}

#[derive(Error, Debug)]
pub enum XsltError {
    #[error("XML parsing error: {0}")]
    XmlParse(#[from] roxmltree::Error),

    #[error("Quick-XML error: {0}")]
    QuickXml(#[from] quick_xml::Error),

    #[error("XPath evaluation error: {0}")]
    XPath(#[from] petty_xpath1::XPathError),

    #[error("Template compilation error: {0}")]
    Compilation(String),

    #[error("Template execution error: {0}")]
    Execution(String),

    #[error("Invalid style property '{property}': {message}")]
    InvalidStyle { property: String, message: String },

    #[error("Template parse error: {0}")]
    TemplateParse(String),

    #[error("Template render error: {0}")]
    TemplateRender(String),

    #[error("XPath parse error in '{0}': {1}")]
    XPathParse(String, String),

    #[error("Template structure error: {message}")]
    TemplateStructure { message: String, location: Location },

    #[error("Template syntax error: {msg} at {location}")]
    TemplateSyntax { msg: String, location: Location },

    #[error("Style error: {0}")]
    Style(String),

    #[error("UTF-8 encoding error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("UTF-8 string error: {0}")]
    Utf8Str(#[from] std::str::Utf8Error),

    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Float parsing error '{0}': {1}")]
    FloatParse(String, std::num::ParseFloatError),
}

impl From<petty_style::StyleParseError> for XsltError {
    fn from(e: petty_style::StyleParseError) -> Self {
        XsltError::Style(e.to_string())
    }
}

impl From<XsltError> for TemplateError {
    fn from(err: XsltError) -> Self {
        match err {
            XsltError::XmlParse(e) => TemplateError::ParseError(e.to_string()),
            XsltError::QuickXml(e) => TemplateError::ParseError(e.to_string()),
            XsltError::XPath(e) => TemplateError::ExecutionError(e.to_string()),
            XsltError::Compilation(s) => TemplateError::ParseError(s),
            XsltError::Execution(s) => TemplateError::ExecutionError(s),
            XsltError::InvalidStyle { property, message } => TemplateError::ParseError(format!(
                "Invalid style property '{}': {}",
                property, message
            )),
            XsltError::TemplateParse(s) => TemplateError::ParseError(s),
            XsltError::TemplateRender(s) => TemplateError::ExecutionError(s),
            XsltError::XPathParse(expr, msg) => {
                TemplateError::ParseError(format!("XPath parse error in '{}': {}", expr, msg))
            }
            XsltError::TemplateStructure { message, location } => TemplateError::ParseError(
                format!("Template structure error at {}: {}", location, message),
            ),
            XsltError::TemplateSyntax { msg, location } => {
                TemplateError::ParseError(format!("Template syntax error at {}: {}", location, msg))
            }
            XsltError::Style(s) => TemplateError::ParseError(s),
            XsltError::Utf8(e) => TemplateError::ParseError(e.to_string()),
            XsltError::Utf8Str(e) => TemplateError::ParseError(e.to_string()),
            XsltError::JsonParse(e) => TemplateError::ParseError(e.to_string()),
            XsltError::FloatParse(s, e) => {
                TemplateError::ParseError(format!("Float parsing error '{}': {}", s, e))
            }
        }
    }
}

impl From<quick_xml::events::attributes::AttrError> for XsltError {
    fn from(e: quick_xml::events::attributes::AttrError) -> Self {
        XsltError::QuickXml(quick_xml::Error::InvalidAttr(e))
    }
}
