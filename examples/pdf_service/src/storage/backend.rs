use async_trait::async_trait;
use uuid::Uuid;

/// Storage backend for PDF files
#[async_trait]
pub trait Storage: Send + Sync {
    /// Upload a PDF file and return its download URL
    async fn upload(&self, job_id: Uuid, file_path: &str) -> Result<String, String>;

    /// Download a PDF file by job ID
    async fn download(&self, job_id: Uuid) -> Result<Vec<u8>, String>;

    /// Check if a file exists for the given job ID
    async fn exists(&self, job_id: Uuid) -> bool;

    /// Delete a file for the given job ID (for cleanup)
    async fn delete(&self, job_id: Uuid) -> Result<(), String>;
}
