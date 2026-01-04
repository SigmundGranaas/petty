use crate::error::Result;
use crate::jobs::{JobQueue, JobResult};
use crate::pipeline::PipelineManager;
use crate::storage::Storage;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

/// Background worker that processes async PDF generation jobs
pub struct Worker {
    job_queue: Arc<dyn JobQueue>,
    pipeline_manager: Arc<PipelineManager>,
    storage: Arc<dyn Storage>,
    poll_interval: Duration,
    worker_id: usize,
}

impl Worker {
    pub fn new(
        job_queue: Arc<dyn JobQueue>,
        pipeline_manager: Arc<PipelineManager>,
        storage: Arc<dyn Storage>,
        poll_interval: Duration,
        worker_id: usize,
    ) -> Self {
        Self {
            job_queue,
            pipeline_manager,
            storage,
            poll_interval,
            worker_id,
        }
    }

    /// Run the worker loop indefinitely
    pub async fn run(self) {
        tracing::info!("Worker {} started", self.worker_id);
        let mut ticker = interval(self.poll_interval);

        loop {
            ticker.tick().await;

            match self.process_next_job().await {
                Ok(true) => {
                    // Job processed successfully, immediately check for more work
                    // Reset ticker to avoid waiting
                    ticker.reset();
                }
                Ok(false) => {
                    // No jobs available, wait for next tick
                }
                Err(e) => {
                    tracing::error!("Worker {} error: {}", self.worker_id, e);
                }
            }
        }
    }

    /// Process the next available job
    /// Returns Ok(true) if a job was processed, Ok(false) if no jobs available
    pub async fn process_next_job(&self) -> Result<bool> {
        // 1. Dequeue job (atomic operation)
        let Some(job) = self.job_queue.dequeue().await? else {
            return Ok(false);
        };

        tracing::info!(
            "Worker {} processing job {} (template: {})",
            self.worker_id,
            job.id,
            job.template
        );

        // 2. Get compiled pipeline for template
        let pipeline = match self.pipeline_manager.get_pipeline(&job.template).await {
            Some(p) => p,
            None => {
                let error = format!("Template '{}' not found", job.template);
                self.job_queue.fail_job(job.id, error).await?;
                return Ok(true);
            }
        };

        // 3. Generate temporary file path for PDF output
        let temp_path = format!("/tmp/pdf-service-job-{}.pdf", job.id);

        // 4. Generate PDF (sync operation wrapped in spawn_blocking)
        let temp_path_clone = temp_path.clone();
        let data = job.data.clone();
        let pipeline_clone = pipeline.clone();

        let generation_result = tokio::task::spawn_blocking(move || {
            pipeline_clone.generate_to_file(vec![data].into_iter(), &temp_path_clone)
        })
        .await
        .map_err(|e| crate::error::ServiceError::Internal(format!("Task join error: {}", e)))?;

        match generation_result {
            Ok(_) => {
                // 5. Get file size
                let file_size = match tokio::fs::metadata(&temp_path).await {
                    Ok(meta) => meta.len() as i64,
                    Err(e) => {
                        let error = format!("Failed to read generated file: {}", e);
                        self.job_queue.fail_job(job.id, error).await?;
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        return Ok(true);
                    }
                };

                // 6. Upload to storage
                let upload_result = self.storage.upload(job.id, &temp_path).await;

                // 7. Cleanup temp file
                let _ = tokio::fs::remove_file(&temp_path).await;

                match upload_result {
                    Ok(download_url) => {
                        // 8. Mark job as completed
                        self.job_queue
                            .complete_job(
                                job.id,
                                JobResult {
                                    download_url,
                                    file_size,
                                },
                            )
                            .await?;

                        tracing::info!(
                            "Worker {} completed job {} ({} bytes)",
                            self.worker_id,
                            job.id,
                            file_size
                        );
                    }
                    Err(e) => {
                        let error = format!("Failed to upload PDF: {}", e);
                        self.job_queue.fail_job(job.id, error).await?;
                    }
                }
            }
            Err(e) => {
                // PDF generation failed
                let error = format!("PDF generation failed: {}", e);
                self.job_queue.fail_job(job.id, error).await?;

                // Cleanup temp file if it exists
                let _ = tokio::fs::remove_file(&temp_path).await;
            }
        }

        Ok(true)
    }
}
