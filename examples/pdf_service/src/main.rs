use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use pdf_service::{
    api,
    config::Config,
    jobs::{PostgresJobQueue, Worker},
    middleware::auth_middleware,
    pipeline::PipelineManager,
    state::AppState,
    storage::FilesystemStorage,
};
use std::{sync::Arc, time::Duration};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    init_tracing();

    tracing::info!("Starting PDF Service...");

    // Load configuration
    let config = Config::load()?;
    tracing::info!("Configuration loaded");

    // Initialize database
    let database_url = Config::database_url();
    let job_queue = PostgresJobQueue::new(&database_url, config.database.max_connections).await?;

    // Run migrations
    job_queue.run_migrations().await?;

    // Health check
    job_queue.health_check().await?;
    tracing::info!("Database connected and migrations complete");

    let job_queue: Arc<dyn pdf_service::jobs::JobQueue> = Arc::new(job_queue);

    // Initialize storage
    let storage = FilesystemStorage::new(config.storage.path.clone())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize storage: {}", e))?;
    let storage: Arc<dyn pdf_service::storage::Storage> = Arc::new(storage);
    tracing::info!("Storage initialized at {}", config.storage.path.display());

    // Initialize pipeline manager
    let pipeline_manager = PipelineManager::new(
        config.pipeline.template_dir.clone(),
        config.pipeline.worker_threads,
        config.pipeline.render_buffer_size,
    )
    .await?;
    tracing::info!("Pipeline manager initialized");

    // Create application state
    let app_state = AppState::new(
        pipeline_manager,
        job_queue.clone(),
        storage.clone(),
        config.clone(),
    );

    // Spawn background workers
    spawn_workers(&app_state).await;

    // Build router
    let app = build_router(app_state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("PDF Service listening on {}", addr);
    tracing::info!("Endpoints:");
    tracing::info!("  - POST /api/v1/generate (sync)");
    tracing::info!("  - POST /api/v1/jobs (async create)");
    tracing::info!("  - GET  /api/v1/jobs/:id (async status)");
    tracing::info!("  - GET  /api/v1/jobs/:id/download (async download)");
    tracing::info!("  - GET  /health");

    axum::serve(listener, app).await?;

    Ok(())
}

fn build_router(state: AppState) -> Router {
    // API routes (with auth)
    let api_routes = Router::new()
        .route("/api/v1/generate", post(api::generate_sync))
        .route("/api/v1/jobs", post(api::create_job))
        .route("/api/v1/jobs/:id", get(api::get_job_status))
        .route("/api/v1/jobs/:id/download", get(api::download_job_result))
        .layer(middleware::from_fn(auth_middleware));

    Router::new()
        // Health check (no auth)
        .route("/health", get(api::health_check))
        // Merge API routes with auth
        .merge(api_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn spawn_workers(state: &AppState) {
    let worker_count = state.config.concurrency.worker_count;
    let poll_interval = Duration::from_millis(state.config.concurrency.worker_poll_interval_ms);

    tracing::info!("Spawning {} background workers", worker_count);

    for worker_id in 0..worker_count {
        let worker = Worker::new(
            state.job_queue.clone(),
            state.pipeline_manager.clone(),
            state.storage.clone(),
            poll_interval,
            worker_id,
        );

        tokio::spawn(async move {
            worker.run().await;
        });
    }

    tracing::info!("Background workers started");
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,pdf_service=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
