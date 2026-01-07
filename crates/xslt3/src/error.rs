use thiserror::Error;

#[derive(Error, Debug)]
pub enum Xslt3Error {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Compile error: {0}")]
    Compile(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Type error: {0}")]
    Type(String),

    #[error("XPath error: {0}")]
    XPath(#[from] petty_xpath31::XPath31Error),

    #[error("XSLT 1.0 error: {0}")]
    Xslt1(#[from] petty_xslt::error::XsltError),

    #[error("Streaming error: {0}")]
    Streaming(String),

    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    #[error("Package error: {0}")]
    Package(String),

    #[error("Dynamic error [{code}]: {message}")]
    Dynamic { code: String, message: String },

    #[error("Circular import detected: {0}")]
    CircularImport(String),

    #[error("Import error for '{href}': {message}")]
    Import { href: String, message: String },

    #[error("Resource error: {0}")]
    Resource(String),
}

impl Xslt3Error {
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    pub fn compile(msg: impl Into<String>) -> Self {
        Self::Compile(msg.into())
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::Runtime(msg.into())
    }

    pub fn type_error(msg: impl Into<String>) -> Self {
        Self::Type(msg.into())
    }

    pub fn streaming(msg: impl Into<String>) -> Self {
        Self::Streaming(msg.into())
    }

    pub fn assertion(msg: impl Into<String>) -> Self {
        Self::AssertionFailed(msg.into())
    }

    pub fn package(msg: impl Into<String>) -> Self {
        Self::Package(msg.into())
    }

    pub fn dynamic(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Dynamic {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn circular_import(uri: impl Into<String>) -> Self {
        Self::CircularImport(uri.into())
    }

    pub fn import(href: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Import {
            href: href.into(),
            message: message.into(),
        }
    }

    pub fn resource(msg: impl Into<String>) -> Self {
        Self::Resource(msg.into())
    }
}

impl From<petty_traits::ResourceError> for Xslt3Error {
    fn from(err: petty_traits::ResourceError) -> Self {
        Self::Resource(err.to_string())
    }
}
