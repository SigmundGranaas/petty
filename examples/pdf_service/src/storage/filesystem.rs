use crate::storage::backend::Storage;
use async_trait::async_trait;
use std::path::PathBuf;
use uuid::Uuid;

/// Filesystem-based storage backend
pub struct FilesystemStorage {
    base_path: PathBuf,
}

impl FilesystemStorage {
    pub async fn new(base_path: PathBuf) -> Result<Self, String> {
        // Create storage directory if it doesn't exist
        tokio::fs::create_dir_all(&base_path)
            .await
            .map_err(|e| format!("Failed to create storage directory: {}", e))?;

        Ok(Self { base_path })
    }

    fn get_file_path(&self, job_id: Uuid) -> PathBuf {
        self.base_path.join(format!("{}.pdf", job_id))
    }
}

#[async_trait]
impl Storage for FilesystemStorage {
    async fn upload(&self, job_id: Uuid, source_path: &str) -> Result<String, String> {
        let dest_path = self.get_file_path(job_id);

        // Copy file to storage location
        tokio::fs::copy(source_path, &dest_path)
            .await
            .map_err(|e| format!("Failed to copy file to storage: {}", e))?;

        // Return relative URL for download
        let download_url = format!("/api/v1/jobs/{}/download", job_id);
        Ok(download_url)
    }

    async fn download(&self, job_id: Uuid) -> Result<Vec<u8>, String> {
        let file_path = self.get_file_path(job_id);

        tokio::fs::read(&file_path)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))
    }

    async fn exists(&self, job_id: Uuid) -> bool {
        let file_path = self.get_file_path(job_id);
        file_path.exists()
    }

    async fn delete(&self, job_id: Uuid) -> Result<(), String> {
        let file_path = self.get_file_path(job_id);

        if file_path.exists() {
            tokio::fs::remove_file(&file_path)
                .await
                .map_err(|e| format!("Failed to delete file: {}", e))?;
        }

        Ok(())
    }
}
