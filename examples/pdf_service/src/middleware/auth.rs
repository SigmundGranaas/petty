use crate::error::ServiceError;
use axum::{extract::Request, http::HeaderMap, middleware::Next, response::Response};

/// API key authentication middleware
pub async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ServiceError> {
    let api_key = crate::config::Config::api_key();

    // Extract API key from X-API-Key header
    let request_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or(ServiceError::Unauthorized)?;

    // Validate API key
    if request_key != api_key {
        return Err(ServiceError::Unauthorized);
    }

    Ok(next.run(request).await)
}
