use crate::error::{Result, ServiceError};
use crate::jobs::models::{Job, JobResult, JobSpec};
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Job queue abstraction for async PDF generation
#[async_trait]
pub trait JobQueue: Send + Sync {
    /// Add a new job to the queue
    async fn enqueue(&self, spec: JobSpec) -> Result<Uuid>;

    /// Get the next pending job and mark it as processing (atomic)
    /// Returns None if no jobs are available
    async fn dequeue(&self) -> Result<Option<Job>>;

    /// Get job status by ID
    async fn get_job(&self, id: Uuid) -> Result<Option<Job>>;

    /// Mark job as completed with result
    async fn complete_job(&self, id: Uuid, result: JobResult) -> Result<()>;

    /// Mark job as failed with error message
    async fn fail_job(&self, id: Uuid, error: String) -> Result<()>;
}

/// PostgreSQL implementation of JobQueue
pub struct PostgresJobQueue {
    pool: PgPool,
}

impl PostgresJobQueue {
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .map_err(ServiceError::Database)?;

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        let migration_sql = include_str!("../../migrations/001_init.sql");

        // Use raw_sql for multi-statement migrations
        sqlx::raw_sql(migration_sql)
            .execute(&self.pool)
            .await
            .map_err(ServiceError::Database)?;

        tracing::info!("Database migrations completed");
        Ok(())
    }

    /// Check database connection health
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(ServiceError::Database)?;
        Ok(())
    }
}

#[async_trait]
impl JobQueue for PostgresJobQueue {
    async fn enqueue(&self, spec: JobSpec) -> Result<Uuid> {
        let id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO jobs (id, template, data, status, callback_url)
            VALUES ($1, $2, $3, 'pending', $4)
            "#,
        )
        .bind(id)
        .bind(&spec.template)
        .bind(&spec.data)
        .bind(&spec.callback_url)
        .execute(&self.pool)
        .await
        .map_err(ServiceError::Database)?;

        tracing::info!("Job {} enqueued for template '{}'", id, spec.template);
        Ok(id)
    }

    async fn dequeue(&self) -> Result<Option<Job>> {
        // Use SELECT FOR UPDATE SKIP LOCKED for efficient, concurrent-safe dequeue
        let row = sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'processing', started_at = NOW()
            WHERE id = (
                SELECT id FROM jobs
                WHERE status = 'pending'
                ORDER BY created_at
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(ServiceError::Database)?;

        if let Some(row) = row {
            let job = Job {
                id: row.get("id"),
                template: row.get("template"),
                data: row.get("data"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                download_url: row.get("download_url"),
                file_size: row.get("file_size"),
                error_message: row.get("error_message"),
                callback_url: row.get("callback_url"),
            };

            tracing::debug!("Dequeued job {} for template '{}'", job.id, job.template);
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn get_job(&self, id: Uuid) -> Result<Option<Job>> {
        let row = sqlx::query(
            r#"
            SELECT * FROM jobs WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ServiceError::Database)?;

        if let Some(row) = row {
            Ok(Some(Job {
                id: row.get("id"),
                template: row.get("template"),
                data: row.get("data"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                download_url: row.get("download_url"),
                file_size: row.get("file_size"),
                error_message: row.get("error_message"),
                callback_url: row.get("callback_url"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn complete_job(&self, id: Uuid, result: JobResult) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'completed',
                completed_at = NOW(),
                download_url = $2,
                file_size = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(&result.download_url)
        .bind(result.file_size)
        .execute(&self.pool)
        .await
        .map_err(ServiceError::Database)?;

        tracing::info!("Job {} completed successfully", id);
        Ok(())
    }

    async fn fail_job(&self, id: Uuid, error: String) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE jobs
            SET status = 'failed',
                error_message = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(&error)
        .execute(&self.pool)
        .await
        .map_err(ServiceError::Database)?;

        tracing::warn!("Job {} failed: {}", id, error);
        Ok(())
    }
}
