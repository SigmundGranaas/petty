//! Defines the unified, rich error types for all parsing operations.
use std::fmt;
use std::num::ParseFloatError;
use std::string::FromUtf8Error;
use thiserror::Error;

/// A struct to hold precise error location information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub col: usize,
}

impl From<(usize, usize)> for Location {
    fn from((line, col): (usize, usize)) -> Self {
        Self { line, col }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line, self.col)
    }
}

/// The main error enum for all parsing operations within the engine.
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("XML parsing error (quick_xml) at {location}: {source}")]
    Xml {
        source: quick_xml::Error,
        location: Location,
    },

    #[error("XML parsing error (roxmltree): {0}")]
    Roxmltree(#[from] roxmltree::Error),

    #[error("XML attribute parsing error: {0}")]
    XmlAttr(#[from] quick_xml::events::attributes::AttrError),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] FromUtf8Error),

    #[error("UTF-8 conversion error from str: {0}")]
    StrUtf8(#[from] std::str::Utf8Error),

    #[error("Failed to parse string '{1}' as a number: {0}")]
    FloatParse(#[source] ParseFloatError, String),

    #[error("Template syntax error: {msg} ({location})")]
    TemplateSyntax { msg: String, location: Location },

    #[error("Invalid style property '{property}' with value '{value}' at {location}: {message}")]
    InvalidStyleProperty {
        property: String,
        value: String,
        message: String,
        location: Location,
    },

    #[error("Template structure error at {location}: {message}")]
    TemplateStructure {
        message: String,
        location: Location,
    },

    #[error("Template rendering error (e.g., Handlebars): {0}")]
    TemplateRender(String),

    #[error("Template parsing error: {0}")]
    TemplateParse(String),

    #[error("XPath parsing error: '{1}' in expression '{0}'")]
    XPathParse(String, String),

    #[error("Failed to parse value: {0}")]
    Nom(String),
}

// Manual impl to avoid location issues with `?`
impl From<quick_xml::Error> for ParseError {
    fn from(e: quick_xml::Error) -> Self {
        ParseError::Xml {
            source: e,
            location: Location { line: 0, col: 0 }, // Location is unknown here
        }
    }
}