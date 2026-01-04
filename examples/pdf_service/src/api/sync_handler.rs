use crate::error::{Result, ServiceError};
use crate::state::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub template: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub message: String,
}

/// Synchronous PDF generation endpoint
/// Returns PDF bytes immediately
pub async fn generate_sync(
    State(state): State<AppState>,
    Json(req): Json<GenerateRequest>,
) -> Result<impl IntoResponse> {
    tracing::info!("Sync generation request for template '{}'", req.template);

    // 1. Acquire semaphore permit (blocks if too many concurrent requests)
    let _permit = state
        .sync_semaphore
        .acquire()
        .await
        .map_err(|_| ServiceError::ServiceOverloaded)?;

    // 2. Get compiled pipeline from cache
    let pipeline = state
        .pipeline_manager
        .get_pipeline(&req.template)
        .await
        .ok_or_else(|| ServiceError::TemplateNotFound(req.template.clone()))?;

    // 3. Generate PDF using async generate with in-memory cursor
    let writer = std::io::Cursor::new(Vec::new());

    // Call async generate directly - it handles spawn_blocking internally
    let final_writer = pipeline
        .generate(vec![req.data].into_iter(), writer)
        .await
        .map_err(|e| ServiceError::GenerationFailed(e.to_string()))?;

    // Extract bytes from cursor
    let pdf_bytes = final_writer.into_inner();

    tracing::info!(
        "Sync generation completed for template '{}' ({} bytes)",
        req.template,
        pdf_bytes.len()
    );

    // 4. Return PDF as response with proper headers
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"output.pdf\"",
            ),
        ],
        pdf_bytes,
    ))
}
