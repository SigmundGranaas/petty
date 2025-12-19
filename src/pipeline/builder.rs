use super::config::{GenerationMode, PdfBackend, PipelineCacheConfig};
use super::orchestrator::DocumentPipeline;
use petty_core::core::layout::fonts::SharedFontLibrary;
use petty_core::error::PipelineError;
use crate::executor::ExecutorImpl;
#[cfg(feature = "rayon-executor")]
use crate::executor::RayonExecutor;
#[cfg(not(feature = "rayon-executor"))]
use crate::executor::SyncExecutor;
use petty_core::parser::json::processor::JsonParser;
use petty_core::parser::processor::{TemplateFeatures, TemplateParser};
use petty_core::parser::xslt::processor::XsltParser;
use crate::pipeline::context::PipelineContext;
use crate::pipeline::provider::metadata::MetadataGeneratingProvider;
use crate::pipeline::provider::passthrough::PassThroughProvider;
use crate::pipeline::provider::Provider;
use crate::pipeline::renderer::composing::ComposingRenderer;
use crate::pipeline::renderer::streaming::SinglePassStreamingRenderer;
use crate::pipeline::renderer::Renderer;
use crate::resource::FilesystemResourceProvider;
use crate::templating::Template;
use petty_core::traits::ResourceProvider;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A builder for creating a `DocumentPipeline`.
pub struct PipelineBuilder {
    template_features: Option<TemplateFeatures>,
    pdf_backend: PdfBackend,
    // Use the thread-safe SharedFontLibrary instead of FontManager
    font_library: SharedFontLibrary,
    resource_provider: Arc<dyn ResourceProvider>,
    executor: ExecutorImpl,
    generation_mode: GenerationMode,
    cache_config: PipelineCacheConfig,
    debug: bool,
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        // SharedFontLibrary::default() loads fallback fonts automatically
        let font_library = SharedFontLibrary::default();

        // Default to filesystem resources with current directory as base
        let resource_provider: Arc<dyn ResourceProvider> =
            Arc::new(FilesystemResourceProvider::new("."));

        // Default to Rayon executor with auto-detected parallelism
        #[cfg(feature = "rayon-executor")]
        let executor = ExecutorImpl::Rayon(RayonExecutor::new());

        #[cfg(not(feature = "rayon-executor"))]
        let executor = ExecutorImpl::Sync(SyncExecutor::new());

        Self {
            template_features: None,
            pdf_backend: Default::default(),
            font_library,
            resource_provider,
            executor,
            generation_mode: Default::default(),
            cache_config: Default::default(),
            debug: false,
        }
    }
}

impl PipelineBuilder {
    /// Creates a new `PipelineBuilder` with default settings and fallback fonts loaded.
    pub fn new() -> Self {
        Default::default()
    }

    /// Configures the pipeline by loading a template from a file.
    /// The template language (XSLT, JSON) is inferred from the file extension.
    pub fn with_template_file<P: AsRef<Path>>(mut self, path: P) -> Result<Self, PipelineError> {
        let path_ref = path.as_ref();
        let extension = path_ref
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let resource_base_path = path_ref
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let template_source = fs::read_to_string(path_ref).map_err(|e| {
            PipelineError::Io(io::Error::new(
                e.kind(),
                format!("Failed to read template from '{}': {}", path_ref.display(), e),
            ))
        })?;

        let parser = self.get_parser_for_extension(extension)?;
        self.template_features = Some(parser.parse(&template_source, resource_base_path)?);
        Ok(self)
    }

    /// Configures the pipeline with a template from a string.
    /// The `extension` argument is used to select the correct parser ("json", "xslt", etc.).
    pub fn with_template_source(
        mut self,
        source: &str,
        extension: &str,
    ) -> Result<Self, PipelineError> {
        let resource_base_path = PathBuf::new(); // No base path for string-based templates
        let parser = self.get_parser_for_extension(extension)?;
        self.template_features = Some(parser.parse(source, resource_base_path)?);
        Ok(self)
    }

    /// Configures the pipeline with a programmatically-built `Template` object.
    pub fn with_template_object(mut self, template: Template) -> Result<Self, PipelineError> {
        let template_source = template.to_json()?;
        let parser = JsonParser;
        let resource_base_path = PathBuf::new();
        self.template_features = Some(parser.parse(&template_source, resource_base_path)?);
        Ok(self)
    }

    /// Scans the host system for installed fonts and adds them to the pipeline's font database.
    /// This is the recommended way to get broad font support.
    pub fn with_system_fonts(mut self, system_fonts: bool) -> Self {
        self.font_library = self.font_library.with_system_fonts(system_fonts);
        self
    }

    /// Scans a directory for font files (`.ttf`, `.otf`, etc.) and adds them to the font database.
    /// Call this for any custom fonts not installed on the system.
    pub fn with_font_dir<P: AsRef<Path>>(self, path: P) -> Self {
        self.font_library.add_font_dir(path.as_ref());
        self
    }

    /// Selects the PDF rendering backend to use.
    pub fn with_pdf_backend(mut self, backend: PdfBackend) -> Self {
        self.pdf_backend = backend;
        self
    }

    /// Selects the document generation strategy.
    /// See `GenerationMode` for details on each option.
    pub fn with_generation_mode(mut self, mode: GenerationMode) -> Self {
        self.generation_mode = mode;
        self
    }

    /// Configures the memory management and caching behavior of the pipeline.
    pub fn with_cache_config(mut self, config: PipelineCacheConfig) -> Self {
        self.cache_config = config;
        self
    }

    /// Enables debug features, such as dumping the layout IR tree.
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Sets a custom resource provider for loading images and other external resources.
    ///
    /// By default, the pipeline uses `FilesystemResourceProvider` with the current directory
    /// as the base path. Use this method to provide:
    /// - A filesystem provider with a different base path
    /// - An in-memory provider for embedded resources
    /// - A custom provider implementation
    pub fn with_resource_provider(mut self, provider: Arc<dyn ResourceProvider>) -> Self {
        self.resource_provider = provider;
        self
    }

    /// Sets a custom executor for controlling parallelism and task execution.
    ///
    /// By default, the pipeline uses:
    /// - `RayonExecutor` when the `rayon-executor` feature is enabled (default)
    /// - `SyncExecutor` (sequential) when Rayon is not available
    ///
    /// Use this method to provide a custom executor implementation or to control
    /// the level of parallelism explicitly.
    pub fn with_executor(mut self, executor: ExecutorImpl) -> Self {
        self.executor = executor;
        self
    }

    /// Consumes the builder and creates the `DocumentPipeline`.
    /// This is where the generation strategy is selected and instantiated.
    pub fn build(mut self) -> Result<DocumentPipeline, PipelineError> {
        let template_features = self.template_features.take().ok_or_else(|| {
            PipelineError::Config(
                "No template has been configured. Use `with_template_file` or `with_template_object`."
                    .to_string(),
            )
        })?;

        let (provider, renderer) = self.select_components(&template_features)?;

        let context = Arc::new(PipelineContext {
            compiled_template: template_features.main_template,
            role_templates: Arc::new(template_features.role_templates),
            // Pass the Arc-wrapped library
            font_library: Arc::new(self.font_library),
            resource_provider: self.resource_provider,
            executor: self.executor,
            cache_config: self.cache_config,
        });

        Ok(DocumentPipeline::new(provider, renderer, context))
    }

    fn select_components(
        &self,
        features: &TemplateFeatures,
    ) -> Result<(Provider, Renderer), PipelineError> {
        let provider: Provider;
        let renderer: Renderer;

        match self.generation_mode {
            GenerationMode::ForceStreaming => {
                log::info!("Forcing Streaming pipeline.");
                provider = Provider::PassThrough(PassThroughProvider);
                renderer = Renderer::Streaming(SinglePassStreamingRenderer::new(self.pdf_backend));
            }
            GenerationMode::Auto => {
                let flags = features.main_template.features();
                if !features.role_templates.is_empty()
                    || flags.uses_index_function
                    || flags.has_table_of_contents
                    || flags.has_page_number_placeholders
                    || flags.has_internal_links
                {
                    log::info!(
                        "Template uses advanced features. Selecting Metadata Generating pipeline."
                    );
                    provider = Provider::Metadata(MetadataGeneratingProvider);
                    renderer = Renderer::Composing(ComposingRenderer);
                } else {
                    log::info!("Template is streamable. Selecting simple Streaming pipeline.");
                    provider = Provider::PassThrough(PassThroughProvider);
                    renderer = Renderer::Streaming(SinglePassStreamingRenderer::new(self.pdf_backend));
                }
            }
        }

        Ok((provider, renderer))
    }

    fn get_parser_for_extension(
        &self,
        extension: &str,
    ) -> Result<Box<dyn TemplateParser>, PipelineError> {
        match extension {
            "xslt" | "xsl" | "fo" => Ok(Box::new(XsltParser)),
            "json" => Ok(Box::new(JsonParser)),
            _ => Err(PipelineError::Config(format!(
                "Unsupported template file extension: .{}",
                extension
            ))),
        }
    }
}