// FILE: src/parser/mod.rs
// src/parser/mod.rs
use std::fmt;
use std::num::ParseFloatError;
use std::string::FromUtf8Error;
use thiserror::Error;

// NEW: A struct to hold error location information.
#[derive(Debug, Clone)]
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
        write!(f, "at line {}, column {}", self.line, self.col)
    }
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("XML parsing error: {0}")]
    XmlParse(#[from] quick_xml::Error),

    #[error("XML attribute parsing error: {0}")]
    XmlAttr(#[from] quick_xml::events::attributes::AttrError),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] FromUtf8Error),

    #[error("Failed to parse string '{1}' as a number: {0}")]
    FloatParse(ParseFloatError, String),

    #[error("Template parsing error: {0}")]
    TemplateParse(String),

    // NEW: More specific error for syntax issues with location.
    #[error("Template syntax error: {msg} ({location})")]
    TemplateSyntax { msg: String, location: Location },

    #[error("Template rendering error (e.g., Handlebars): {0}")]
    TemplateRender(String),

    #[error("XPath parsing error: '{1}' in expression '{0}'")]
    XPathParse(String, String),
}

pub mod json;
pub mod processor;
pub mod style;
pub mod xslt;