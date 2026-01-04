use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Template '{0}' not found")]
    TemplateNotFound(String),

    #[error("PDF generation failed: {0}")]
    GenerationFailed(String),

    #[error("Service overloaded, please try again later")]
    ServiceOverloaded,

    #[error("Job not found")]
    JobNotFound,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Pipeline error: {0}")]
    Pipeline(#[from] petty::PipelineError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::TemplateNotFound(_) => (
                StatusCode::BAD_REQUEST,
                "TemplateNotFound",
                self.to_string(),
            ),
            Self::GenerationFailed(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "GenerationFailed",
                self.to_string(),
            ),
            Self::ServiceOverloaded => (
                StatusCode::SERVICE_UNAVAILABLE,
                "ServiceOverloaded",
                self.to_string(),
            ),
            Self::JobNotFound => (StatusCode::NOT_FOUND, "JobNotFound", self.to_string()),
            Self::InvalidRequest(_) => {
                (StatusCode::BAD_REQUEST, "InvalidRequest", self.to_string())
            }
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Unauthorized",
                "Invalid or missing API key".to_string(),
            ),
            Self::Database(_) | Self::Storage(_) | Self::Internal(_) => {
                tracing::error!("Internal error: {}", self);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalError",
                    "An internal error occurred".to_string(),
                )
            }
            Self::Pipeline(ref e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "PipelineError",
                format!("PDF generation error: {}", e),
            ),
            Self::Config(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ConfigError",
                "Configuration error".to_string(),
            ),
        };

        let body = Json(json!({
            "error": code,
            "message": message,
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, ServiceError>;
