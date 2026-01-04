use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Job specification for creating a new PDF generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpec {
    pub template: String,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
}

/// Job status enum
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
pub enum JobStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "processing")]
    Processing,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Pending => write!(f, "pending"),
            JobStatus::Processing => write!(f, "processing"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(JobStatus::Pending),
            "processing" => Ok(JobStatus::Processing),
            "completed" => Ok(JobStatus::Completed),
            "failed" => Ok(JobStatus::Failed),
            _ => Err(format!("Invalid job status: {}", s)),
        }
    }
}

/// Complete job record from database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub template: String,
    pub data: serde_json::Value,
    pub status: String,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    // Result
    pub download_url: Option<String>,
    pub file_size: Option<i64>,
    pub error_message: Option<String>,

    // Metadata
    pub callback_url: Option<String>,
}

/// Job result for completed jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub download_url: String,
    pub file_size: i64,
}

/// API response for job status queries
#[derive(Debug, Serialize, Deserialize)]
pub struct JobStatusResponse {
    pub job_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JobResultInfo>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobResultInfo {
    pub download_url: String,
    pub file_size: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub message: String,
}

impl From<Job> for JobStatusResponse {
    fn from(job: Job) -> Self {
        let result = if job.status == "completed" {
            job.download_url.zip(job.file_size).map(|(url, size)| {
                let expires_at =
                    job.completed_at.unwrap_or(job.updated_at) + chrono::Duration::hours(24);

                JobResultInfo {
                    download_url: url,
                    file_size: size,
                    expires_at,
                }
            })
        } else {
            None
        };

        let error = if job.status == "failed" {
            job.error_message.map(|msg| ErrorInfo { message: msg })
        } else {
            None
        };

        Self {
            job_id: job.id,
            status: job.status,
            created_at: job.created_at,
            updated_at: job.updated_at,
            started_at: job.started_at,
            completed_at: job.completed_at,
            result,
            error,
        }
    }
}

/// API response for job creation
#[derive(Debug, Serialize, Deserialize)]
pub struct JobCreateResponse {
    pub job_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub status_url: String,
}
