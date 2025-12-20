use thiserror::Error;

#[derive(Error, Debug)]
pub enum ComposerError {
    #[error("PDF error: {0}")]
    Pdf(#[from] lopdf::Error),

    #[error("{0}")]
    Other(String),
}
