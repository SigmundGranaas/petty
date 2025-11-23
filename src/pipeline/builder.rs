// src/pipeline/builder.rs
use super::config::{GenerationMode, PdfBackend};
use super::orchestrator::DocumentPipeline;
use crate::core::layout::FontManager;
use crate::error::PipelineError;
use crate::parser::json::processor::JsonParser;
use crate::parser::processor::{TemplateFeatures, TemplateParser};
use crate::parser::xslt::processor::XsltParser;
use crate::pipeline::provider::metadata::MetadataGeneratingProvider;
use crate::pipeline::provider::passthrough::PassThroughProvider;
use crate::pipeline::provider::Provider;
use crate::pipeline::renderer::composing::ComposingRenderer;
use crate::pipeline::renderer::streaming::SinglePassStreamingRenderer;
use crate::pipeline::renderer::Renderer;
use crate::templating::Template;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::pipeline::context::PipelineContext;

/// A builder for creating a `DocumentPipeline`.
pub struct PipelineBuilder {
    template_features: Option<TemplateFeatures>,
    pdf_backend: PdfBackend,
    font_manager: FontManager,
    generation_mode: GenerationMode,
    debug: bool,
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        let font_manager = FontManager::new();
        font_manager.load_fallback_font();
        Self {
            template_features: None,
            pdf_backend: Default::default(),
            font_manager,
            generation_mode: Default::default(),
            debug: false,
        }
    }
}

impl PipelineBuilder {
    /// Creates a new `PipelineBuilder` with default settings and fallback fonts loaded.
    pub fn new() -> Self { Default::default() }

    /// Configures the pipeline by loading a template from a file.
    /// The template language (XSLT, JSON) is inferred from the file extension.
    pub fn with_template_file<P: AsRef<Path>>(mut self, path: P) -> Result<Self, PipelineError> {
        let path_ref = path.as_ref();
        let extension = path_ref.extension().and_then(|s| s.to_str()).unwrap_or("");
        let resource_base_path = path_ref.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let template_source = fs::read_to_string(path_ref).map_err(|e| PipelineError::Io(io::Error::new(e.kind(), format!("Failed to read template from '{}': {}", path_ref.display(), e))))?;

        let parser = self.get_parser_for_extension(extension)?;
        self.template_features = Some(parser.parse(&template_source, resource_base_path)?);
        Ok(self)
    }

    /// Configures the pipeline with a template from a string.
    /// The `extension` argument is used to select the correct parser ("json", "xslt", etc.).
    pub fn with_template_source(mut self, source: &str, extension: &str) -> Result<Self, PipelineError> {
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
    pub fn with_system_fonts(self, system_fonts: bool) -> Self { if system_fonts { self.font_manager.load_system_fonts(); } self }

    /// Scans a directory for font files (`.ttf`, `.otf`, etc.) and adds them to the font database.
    /// Call this for any custom fonts not installed on the system.
    pub fn with_font_dir<P: AsRef<Path>>(self, path: P) -> Self { self.font_manager.load_fonts_from_dir(path.as_ref()); self }

    /// Selects the PDF rendering backend to use.
    pub fn with_pdf_backend(mut self, backend: PdfBackend) -> Self { self.pdf_backend = backend; self }

    /// Selects the document generation strategy.
    /// See `GenerationMode` for details on each option.
    pub fn with_generation_mode(mut self, mode: GenerationMode) -> Self { self.generation_mode = mode; self }

    /// Enables debug features, such as dumping the layout IR tree.
    pub fn with_debug(mut self, debug: bool) -> Self { self.debug = debug; self }

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
            font_manager: Arc::new(self.font_manager),
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
                {
                    log::info!("Template uses advanced features. Selecting Metadata Generating pipeline.");
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

    fn get_parser_for_extension(&self, extension: &str) -> Result<Box<dyn TemplateParser>, PipelineError> {
        match extension {
            "xslt" | "xsl" | "fo" => Ok(Box::new(XsltParser)),
            "json" => Ok(Box::new(JsonParser)),
            _ => Err(PipelineError::Config(format!("Unsupported template file extension: .{}", extension)))
        }
    }
}