use thiserror::Error;

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF generation error: {0}")]
    Pdf(String),
    #[error("Internal PDF library error: {0}")]
    PdfLibError(String),
    #[error("Internal PDF error: {0}")]
    InternalPdfError(String),
    #[error("Template rendering error: {0}")]
    Template(#[from] handlebars::RenderError),
    #[error("Other rendering error: {0}")]
    Other(String),
}

impl From<lopdf::Error> for RenderError {
    fn from(err: lopdf::Error) -> Self {
        RenderError::Pdf(err.to_string())
    }
}

impl From<&str> for RenderError {
    fn from(s: &str) -> Self {
        RenderError::Other(s.to_string())
    }
}
