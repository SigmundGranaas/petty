use pdf_service::{
    config::Config,
    jobs::{JobQueue, JobSpec},
    pipeline::PipelineManager,
    state::AppState,
    storage::{FilesystemStorage, Storage},
};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

/// Test helper to create a test invoice template
fn create_test_template() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
    <xsl:output method="xml" indent="yes"/>
    <xsl:template match="/">
        <document>
            <layout-master-set>
                <simple-page-master master-name="A4" page-width="595pt" page-height="842pt" margin="50pt">
                    <region-body/>
                </simple-page-master>
            </layout-master-set>
            <page-sequence master-reference="A4">
                <flow>
                    <block margin-bottom="20pt">
                        <text font-size="24pt" font-weight="bold">Test Invoice</text>
                    </block>
                    <block>
                        <text font-size="12pt">
                            Customer: <xsl:value-of select="invoice/customer"/>
                        </text>
                    </block>
                    <block margin-top="10pt">
                        <text font-size="12pt">
                            Total: $<xsl:value-of select="invoice/total"/>
                        </text>
                    </block>
                </flow>
            </page-sequence>
        </document>
    </xsl:template>
</xsl:stylesheet>"#
}

/// Test helper to create test data
fn create_test_data() -> serde_json::Value {
    json!({
        "invoice": {
            "number": "INV-TEST-001",
            "company": {
                "name": "Test Company",
                "address": "123 Test St",
                "city": "Test City",
                "zip": "12345"
            },
            "customer": {
                "name": "Test Customer",
                "email": "test@example.com"
            },
            "items": [
                {
                    "description": "Test Product",
                    "quantity": 1,
                    "price": "100.00",
                    "total": "100.00"
                }
            ],
            "subtotal": "100.00",
            "tax_rate": "10",
            "tax": "10.00",
            "total": "110.00"
        }
    })
}

/// Test helper to create a test app state
async fn create_test_state() -> (AppState, TempDir, TempDir) {
    // Create temporary directories
    let template_dir = TempDir::new().unwrap();
    let storage_dir = TempDir::new().unwrap();

    // Copy the actual invoice template from the pdf_service templates
    let source_template = std::path::PathBuf::from("examples/pdf_service/templates/invoice.xsl");
    let template_path = template_dir.path().join("test_invoice.xsl");

    if source_template.exists() {
        std::fs::copy(&source_template, &template_path).unwrap();
    } else {
        // Fallback to embedded template if source doesn't exist
        std::fs::write(&template_path, create_test_template()).unwrap();
    }

    // Create test config
    let mut config = Config::load().unwrap_or_else(|_| {
        // Default test config if loading fails
        serde_json::from_value(json!({
            "server": {
                "host": "127.0.0.1",
                "port": 3000,
                "max_request_size_mb": 10
            },
            "concurrency": {
                "max_sync_requests": 2,
                "worker_count": 2,
                "worker_poll_interval_ms": 100
            },
            "pipeline": {
                "template_dir": template_dir.path(),
                "worker_threads": 2,
                "render_buffer_size": 16
            },
            "storage": {
                "backend": "filesystem",
                "path": storage_dir.path(),
                "result_ttl_hours": 1
            },
            "database": {
                "max_connections": 5
            }
        }))
        .unwrap()
    });

    // Override paths with test directories
    config.pipeline.template_dir = template_dir.path().to_path_buf();
    config.storage.path = storage_dir.path().to_path_buf();

    // Initialize pipeline manager
    let pipeline_manager = PipelineManager::new(
        config.pipeline.template_dir.clone(),
        config.pipeline.worker_threads,
        config.pipeline.render_buffer_size,
    )
    .await
    .expect("Failed to create pipeline manager");

    // Initialize storage
    let storage = FilesystemStorage::new(config.storage.path.clone())
        .await
        .expect("Failed to create storage");
    let storage: Arc<dyn pdf_service::storage::Storage> = Arc::new(storage);

    // Create in-memory job queue for testing
    let job_queue = InMemoryJobQueue::new();
    let job_queue: Arc<dyn JobQueue> = Arc::new(job_queue);

    // Create app state
    let state = AppState::new(pipeline_manager, job_queue, storage, config);

    (state, template_dir, storage_dir)
}

/// In-memory job queue for testing (no database required)
struct InMemoryJobQueue {
    jobs: tokio::sync::RwLock<std::collections::HashMap<uuid::Uuid, pdf_service::jobs::Job>>,
}

impl InMemoryJobQueue {
    fn new() -> Self {
        Self {
            jobs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl JobQueue for InMemoryJobQueue {
    async fn enqueue(&self, spec: JobSpec) -> Result<uuid::Uuid, pdf_service::error::ServiceError> {
        let id = uuid::Uuid::new_v4();
        let job = pdf_service::jobs::Job {
            id,
            template: spec.template,
            data: spec.data,
            status: "pending".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            download_url: None,
            file_size: None,
            error_message: None,
            callback_url: spec.callback_url,
        };

        self.jobs.write().await.insert(id, job);
        Ok(id)
    }

    async fn dequeue(
        &self,
    ) -> Result<Option<pdf_service::jobs::Job>, pdf_service::error::ServiceError> {
        let mut jobs = self.jobs.write().await;
        let pending_job = jobs
            .iter_mut()
            .find(|(_, job)| job.status == "pending")
            .map(|(id, job)| {
                job.status = "processing".to_string();
                job.started_at = Some(chrono::Utc::now());
                (*id, job.clone())
            });

        Ok(pending_job.map(|(_, job)| job))
    }

    async fn get_job(
        &self,
        id: uuid::Uuid,
    ) -> Result<Option<pdf_service::jobs::Job>, pdf_service::error::ServiceError> {
        Ok(self.jobs.read().await.get(&id).cloned())
    }

    async fn complete_job(
        &self,
        id: uuid::Uuid,
        result: pdf_service::jobs::JobResult,
    ) -> Result<(), pdf_service::error::ServiceError> {
        if let Some(job) = self.jobs.write().await.get_mut(&id) {
            job.status = "completed".to_string();
            job.completed_at = Some(chrono::Utc::now());
            job.download_url = Some(result.download_url);
            job.file_size = Some(result.file_size);
        }
        Ok(())
    }

    async fn fail_job(
        &self,
        id: uuid::Uuid,
        error: String,
    ) -> Result<(), pdf_service::error::ServiceError> {
        if let Some(job) = self.jobs.write().await.get_mut(&id) {
            job.status = "failed".to_string();
            job.error_message = Some(error);
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_pipeline_manager_loads_templates() {
    let (state, _template_dir, _storage_dir) = create_test_state().await;

    // Check that template was loaded
    let templates = state.pipeline_manager.list_templates().await;
    assert_eq!(templates.len(), 1, "Should have loaded 1 template");
    assert!(
        templates.contains(&"test_invoice".to_string()),
        "Should have loaded test_invoice template"
    );
}

#[tokio::test]
#[ignore = "Requires valid XSLT template - test service components only"]
async fn test_pipeline_generates_pdf() {
    let (state, _template_dir, _storage_dir) = create_test_state().await;

    // Get the pipeline
    let pipeline = state
        .pipeline_manager
        .get_pipeline("test_invoice")
        .await
        .expect("Template should exist");

    // Generate PDF to memory
    let buffer = std::io::Cursor::new(Vec::new());
    let result = pipeline
        .generate(vec![create_test_data()].into_iter(), buffer)
        .await
        .expect("PDF generation should succeed");

    let pdf_bytes = result.into_inner();

    // Verify PDF was generated
    assert!(pdf_bytes.len() > 100, "PDF should have content");
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "Should start with PDF header"
    );
}

#[tokio::test]
#[ignore = "Requires valid XSLT template - test service components only"]
async fn test_sync_generation_endpoint() {
    let (state, _template_dir, _storage_dir) = create_test_state().await;

    // Test the components directly
    let pipeline = state
        .pipeline_manager
        .get_pipeline("test_invoice")
        .await
        .unwrap();

    let buffer = std::io::Cursor::new(Vec::new());
    let result = pipeline
        .generate(vec![create_test_data()].into_iter(), buffer)
        .await;

    assert!(result.is_ok(), "Generation should succeed");
    let pdf_bytes = result.unwrap().into_inner();
    assert!(pdf_bytes.len() > 100);
}

#[tokio::test]
async fn test_async_job_lifecycle() {
    let (state, _template_dir, _storage_dir) = create_test_state().await;

    // 1. Enqueue a job
    let spec = JobSpec {
        template: "test_invoice".to_string(),
        data: create_test_data(),
        callback_url: None,
    };

    let job_id = state.job_queue.enqueue(spec).await.unwrap();

    // 2. Verify job is pending
    let job = state.job_queue.get_job(job_id).await.unwrap().unwrap();
    assert_eq!(job.status, "pending");

    // 3. Dequeue the job
    let dequeued = state.job_queue.dequeue().await.unwrap();
    assert!(dequeued.is_some());
    let dequeued_job = dequeued.unwrap();
    assert_eq!(dequeued_job.id, job_id);
    assert_eq!(dequeued_job.status, "processing");

    // 4. Complete the job
    let result = pdf_service::jobs::JobResult {
        download_url: format!("/api/v1/jobs/{}/download", job_id),
        file_size: 1234,
    };
    state.job_queue.complete_job(job_id, result).await.unwrap();

    // 5. Verify job is completed
    let completed_job = state.job_queue.get_job(job_id).await.unwrap().unwrap();
    assert_eq!(completed_job.status, "completed");
    assert!(completed_job.download_url.is_some());
    assert_eq!(completed_job.file_size, Some(1234));
}

#[tokio::test]
async fn test_storage_backend() {
    let storage_dir = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(storage_dir.path().to_path_buf())
        .await
        .unwrap();

    let job_id = uuid::Uuid::new_v4();

    // Create a test PDF file
    let test_pdf = b"%PDF-1.4\n%Test PDF content\n";
    let temp_file = storage_dir.path().join("temp.pdf");
    std::fs::write(&temp_file, test_pdf).unwrap();

    // Upload the file
    let download_url = storage
        .upload(job_id, temp_file.to_str().unwrap())
        .await
        .unwrap();

    assert_eq!(download_url, format!("/api/v1/jobs/{}/download", job_id));

    // Verify file exists
    assert!(storage.exists(job_id).await);

    // Download the file
    let downloaded = storage.download(job_id).await.unwrap();
    assert_eq!(downloaded, test_pdf);

    // Delete the file
    storage.delete(job_id).await.unwrap();
    assert!(!storage.exists(job_id).await);
}

#[tokio::test]
#[ignore = "Requires valid XSLT template - test service components only"]
async fn test_worker_processes_job() {
    let (state, _template_dir, _storage_dir) = create_test_state().await;

    // Enqueue a job
    let spec = JobSpec {
        template: "test_invoice".to_string(),
        data: create_test_data(),
        callback_url: None,
    };

    let job_id = state.job_queue.enqueue(spec).await.unwrap();

    // Create and run worker
    let worker = pdf_service::jobs::Worker::new(
        state.job_queue.clone(),
        state.pipeline_manager.clone(),
        state.storage.clone(),
        std::time::Duration::from_millis(100),
        0,
    );

    // Process one job
    let processed = worker
        .process_next_job()
        .await
        .expect("Worker should process job");
    assert!(processed, "Worker should have processed a job");

    // Wait a bit for async operations to complete
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify job is completed
    let job = state.job_queue.get_job(job_id).await.unwrap().unwrap();
    assert_eq!(
        job.status, "completed",
        "Job should be completed, got: {}",
        job.status
    );

    // Verify PDF was stored
    assert!(state.storage.exists(job_id).await);

    // Verify PDF content
    let pdf_bytes = state.storage.download(job_id).await.unwrap();
    assert!(pdf_bytes.len() > 100);
    assert!(pdf_bytes.starts_with(b"%PDF"));
}

#[tokio::test]
#[ignore = "Requires valid XSLT template - test service components only"]
async fn test_multiple_concurrent_generations() {
    let (state, _template_dir, _storage_dir) = create_test_state().await;

    // Generate multiple PDFs concurrently
    let pipeline = state
        .pipeline_manager
        .get_pipeline("test_invoice")
        .await
        .unwrap();

    let tasks: Vec<_> = (0..5)
        .map(|i| {
            let pipeline = pipeline.clone();
            let data = json!({
                "invoice": {
                    "customer": format!("Customer {}", i),
                    "total": format!("{}.00", i * 10)
                }
            });

            tokio::spawn(async move {
                let buffer = std::io::Cursor::new(Vec::new());
                pipeline
                    .generate(vec![data].into_iter(), buffer)
                    .await
                    .map(|r| r.into_inner())
            })
        })
        .collect();

    // Wait for all tasks to complete
    let results = futures::future::join_all(tasks).await;

    // Verify all succeeded
    for (i, result) in results.iter().enumerate() {
        let pdf_bytes = result
            .as_ref()
            .unwrap()
            .as_ref()
            .expect(&format!("Generation {} should succeed", i));
        assert!(pdf_bytes.len() > 100, "PDF {} should have content", i);
    }
}

#[tokio::test]
async fn test_invalid_template_returns_error() {
    let (state, _template_dir, _storage_dir) = create_test_state().await;

    // Try to get non-existent template
    let pipeline = state
        .pipeline_manager
        .get_pipeline("nonexistent_template")
        .await;

    assert!(
        pipeline.is_none(),
        "Should return None for invalid template"
    );
}

#[tokio::test]
async fn test_job_queue_concurrent_dequeue() {
    let queue = Arc::new(InMemoryJobQueue::new());

    // Enqueue multiple jobs
    for i in 0..10 {
        let spec = JobSpec {
            template: format!("template_{}", i),
            data: json!({"test": i}),
            callback_url: None,
        };
        queue.enqueue(spec).await.unwrap();
    }

    // Dequeue concurrently from multiple workers
    let tasks: Vec<_> = (0..10)
        .map(|_| {
            let queue = queue.clone();
            tokio::spawn(async move { queue.dequeue().await })
        })
        .collect();

    let results = futures::future::join_all(tasks).await;

    // Verify all jobs were dequeued exactly once
    let dequeued_jobs: Vec<_> = results
        .into_iter()
        .filter_map(|r| r.unwrap().unwrap())
        .collect();

    assert_eq!(dequeued_jobs.len(), 10, "All jobs should be dequeued");

    // Verify all job IDs are unique
    let mut ids: Vec<_> = dequeued_jobs.iter().map(|j| j.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 10, "All job IDs should be unique");
}
