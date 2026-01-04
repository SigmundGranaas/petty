use crate::error::{Result, ServiceError};
use crate::jobs::{JobCreateResponse, JobSpec, JobStatusResponse};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

/// Create a new async PDF generation job
pub async fn create_job(
    State(state): State<AppState>,
    Json(spec): Json<JobSpec>,
) -> Result<impl IntoResponse> {
    tracing::info!("Job creation request for template '{}'", spec.template);

    // Validate template exists
    if state
        .pipeline_manager
        .get_pipeline(&spec.template)
        .await
        .is_none()
    {
        return Err(ServiceError::TemplateNotFound(spec.template));
    }

    // Enqueue job
    let job_id = state.job_queue.enqueue(spec).await?;

    let response = JobCreateResponse {
        job_id,
        status: "pending".to_string(),
        created_at: chrono::Utc::now(),
        status_url: format!("/api/v1/jobs/{}", job_id),
    };

    tracing::info!("Job {} created and enqueued", job_id);

    Ok((StatusCode::ACCEPTED, Json(response)))
}

/// Get job status
pub async fn get_job_status(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let job = state
        .job_queue
        .get_job(job_id)
        .await?
        .ok_or(ServiceError::JobNotFound)?;

    let response: JobStatusResponse = job.into();

    Ok(Json(response))
}

/// Download completed PDF
pub async fn download_job_result(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    // 1. Get job and verify it's completed
    let job = state
        .job_queue
        .get_job(job_id)
        .await?
        .ok_or(ServiceError::JobNotFound)?;

    if job.status != "completed" {
        return Err(ServiceError::InvalidRequest(format!(
            "Job is not completed (status: {})",
            job.status
        )));
    }

    // 2. Download file from storage
    let pdf_bytes = state
        .storage
        .download(job_id)
        .await
        .map_err(ServiceError::Storage)?;

    tracing::info!("Job {} PDF downloaded ({} bytes)", job_id, pdf_bytes.len());

    // 3. Return PDF with proper headers
    use axum::body::Body;
    use axum::response::Response;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"job-{}.pdf\"", job_id),
        )
        .body(Body::from(pdf_bytes))
        .map_err(|e| ServiceError::Internal(format!("Failed to build response: {}", e)))?;

    Ok(response)
}
