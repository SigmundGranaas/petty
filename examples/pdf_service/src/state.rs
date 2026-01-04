use crate::config::Config;
use crate::jobs::JobQueue;
use crate::pipeline::PipelineManager;
use crate::storage::Storage;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Shared application state accessible to all handlers and workers
#[derive(Clone)]
pub struct AppState {
    /// Manages compiled Petty pipelines (one per template)
    pub pipeline_manager: Arc<PipelineManager>,

    /// Job queue (PostgreSQL-backed)
    pub job_queue: Arc<dyn JobQueue>,

    /// PDF storage backend
    pub storage: Arc<dyn Storage>,

    /// Limits concurrent synchronous PDF generation
    /// Prevents OOM from too many simultaneous render tasks
    pub sync_semaphore: Arc<Semaphore>,

    /// Configuration
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(
        pipeline_manager: PipelineManager,
        job_queue: Arc<dyn JobQueue>,
        storage: Arc<dyn Storage>,
        config: Config,
    ) -> Self {
        let sync_semaphore = Arc::new(Semaphore::new(config.concurrency.max_sync_requests));

        Self {
            pipeline_manager: Arc::new(pipeline_manager),
            job_queue,
            storage,
            sync_semaphore,
            config: Arc::new(config),
        }
    }
}
