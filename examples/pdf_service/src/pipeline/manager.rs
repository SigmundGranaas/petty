use crate::error::{Result, ServiceError};
use petty::{DocumentPipeline, PdfBackend, PipelineBuilder, ProcessingMode};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages Petty DocumentPipelines with template caching
pub struct PipelineManager {
    /// Cached pipelines indexed by template name
    pipelines: RwLock<HashMap<String, Arc<DocumentPipeline>>>,
    template_dir: PathBuf,
    worker_threads: usize,
    render_buffer_size: usize,
}

impl PipelineManager {
    /// Create a new PipelineManager and load all templates
    pub async fn new(
        template_dir: PathBuf,
        worker_threads: usize,
        render_buffer_size: usize,
    ) -> Result<Self> {
        let manager = Self {
            pipelines: RwLock::new(HashMap::new()),
            template_dir,
            worker_threads,
            render_buffer_size,
        };

        // Pre-load all templates at startup
        manager.load_templates().await?;

        Ok(manager)
    }

    /// Get a cached pipeline by template name
    pub async fn get_pipeline(&self, name: &str) -> Option<Arc<DocumentPipeline>> {
        let pipelines = self.pipelines.read().await;
        pipelines.get(name).cloned()
    }

    /// Reload all templates (useful for development/hot-reload)
    pub async fn reload(&self) -> Result<()> {
        let mut pipelines = self.pipelines.write().await;
        pipelines.clear();
        drop(pipelines);

        self.load_templates().await
    }

    /// Load all templates from the template directory
    async fn load_templates(&self) -> Result<()> {
        if !self.template_dir.exists() {
            return Err(ServiceError::Config(format!(
                "Template directory does not exist: {}",
                self.template_dir.display()
            )));
        }

        let mut entries = tokio::fs::read_dir(&self.template_dir).await.map_err(|e| {
            ServiceError::Config(format!("Failed to read template directory: {}", e))
        })?;

        let mut loaded_count = 0;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ServiceError::Config(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Check for XSLT templates (.xsl, .xslt)
            if let Some(ext) = path.extension() {
                if ext == "xsl" || ext == "xslt" {
                    self.load_xslt_template(&path).await?;
                    loaded_count += 1;
                }
            }
        }

        tracing::info!(
            "Loaded {} templates from {}",
            loaded_count,
            self.template_dir.display()
        );

        if loaded_count == 0 {
            tracing::warn!(
                "No templates found in {}. Add .xsl or .xslt files to enable PDF generation.",
                self.template_dir.display()
            );
        }

        Ok(())
    }

    /// Load and compile an XSLT template
    async fn load_xslt_template(&self, path: &Path) -> Result<()> {
        let template_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                ServiceError::Config(format!("Invalid template filename: {}", path.display()))
            })?
            .to_string();

        tracing::debug!(
            "Loading XSLT template '{}' from {}",
            template_name,
            path.display()
        );

        // Build the pipeline
        let pipeline = PipelineBuilder::new()
            .with_template_file(path)
            .map_err(ServiceError::Pipeline)?
            .with_system_fonts(true)
            .with_pdf_backend(PdfBackend::LopdfParallel)
            .with_processing_mode(ProcessingMode::WithMetrics)
            .with_worker_count(self.worker_threads)
            .with_render_buffer_size(self.render_buffer_size)
            .build()
            .map_err(ServiceError::Pipeline)?;

        // Store in cache
        let mut pipelines = self.pipelines.write().await;
        pipelines.insert(template_name.clone(), Arc::new(pipeline));

        tracing::info!(
            "Template '{}' loaded and compiled successfully",
            template_name
        );

        Ok(())
    }

    /// List all available template names
    pub async fn list_templates(&self) -> Vec<String> {
        let pipelines = self.pipelines.read().await;
        pipelines.keys().cloned().collect()
    }
}
