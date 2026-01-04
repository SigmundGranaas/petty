use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub concurrency: ConcurrencyConfig,
    pub pipeline: PipelineConfig,
    pub storage: StorageConfig,
    pub database: DatabaseConfig,
    /// Base path for resolving relative paths (set during loading)
    #[serde(skip)]
    base_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_request_size_mb: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConcurrencyConfig {
    pub max_sync_requests: usize,
    pub worker_count: usize,
    pub worker_poll_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineConfig {
    pub template_dir: PathBuf,
    pub worker_threads: usize,
    pub render_buffer_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub backend: String,
    pub path: PathBuf,
    pub result_ttl_hours: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub max_connections: u32,
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        // Try multiple config file locations in order of preference
        let config_candidates = [
            // 1. Current directory (when running from examples/pdf_service/)
            ("config/default", Some(".")),
            // 2. Workspace root (when running from project root)
            (
                "examples/pdf_service/config/default",
                Some("examples/pdf_service"),
            ),
        ];

        let mut builder = config::Config::builder();
        let mut base_path: Option<PathBuf> = None;

        // Check for environment variable override first
        if let Ok(config_path) = std::env::var("PDF_SERVICE_CONFIG") {
            if !config_path.is_empty() {
                let config_file = format!("{}.toml", config_path);
                if std::path::Path::new(&config_file).exists() {
                    builder = builder.add_source(config::File::with_name(&config_path));
                    // Use parent directory of config file as base
                    base_path = std::path::Path::new(&config_file)
                        .parent()
                        .and_then(|p| p.parent())
                        .map(|p| p.to_path_buf());
                }
            }
        }

        // If no env override found a config, try the candidates
        if base_path.is_none() {
            for (path, base) in &config_candidates {
                let config_file = format!("{}.toml", path);
                if std::path::Path::new(&config_file).exists() {
                    builder = builder.add_source(config::File::with_name(path));
                    base_path = base.map(PathBuf::from);
                    break;
                }
            }
        }

        // Always layer environment variables on top
        builder =
            builder.add_source(config::Environment::with_prefix("PDF_SERVICE").separator("__"));

        let mut config: Config = builder.build()?.try_deserialize()?;
        config.base_path = base_path;

        // Resolve relative paths
        config.resolve_paths();

        Ok(config)
    }

    /// Resolve relative paths in the config based on the base path
    fn resolve_paths(&mut self) {
        if let Some(ref base) = self.base_path {
            // Resolve template_dir
            if self.pipeline.template_dir.is_relative() {
                // Strip the leading "./" or "examples/pdf_service/" prefix if present
                let template_path = self
                    .pipeline
                    .template_dir
                    .strip_prefix("./examples/pdf_service/")
                    .or_else(|_| {
                        self.pipeline
                            .template_dir
                            .strip_prefix("examples/pdf_service/")
                    })
                    .or_else(|_| self.pipeline.template_dir.strip_prefix("./"))
                    .unwrap_or(&self.pipeline.template_dir);
                self.pipeline.template_dir = base.join(template_path);
            }

            // Resolve storage path
            if self.storage.path.is_relative() {
                let storage_path = self
                    .storage
                    .path
                    .strip_prefix("./examples/pdf_service/")
                    .or_else(|_| self.storage.path.strip_prefix("examples/pdf_service/"))
                    .or_else(|_| self.storage.path.strip_prefix("./"))
                    .unwrap_or(&self.storage.path);
                self.storage.path = base.join(storage_path);
            }
        }
    }

    pub fn database_url() -> String {
        std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost:5432/pdf_service".to_string()
        })
    }

    pub fn api_key() -> String {
        std::env::var("API_KEY").unwrap_or_else(|_| "dev-secret-key".to_string())
    }
}
