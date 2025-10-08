use super::config::PdfBackend;
use super::orchestrator::DocumentPipeline;
use crate::core::layout::FontManager;
use crate::error::PipelineError;
use crate::parser::processor::{CompiledTemplate, TemplateParser};
use crate::parser::xslt::processor::XsltParser;
use crate::templating::Template;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::parser::json::processor::JsonParser;

/// A builder for creating a `DocumentPipeline`.
pub struct PipelineBuilder {
    compiled_template: Option<Arc<dyn CompiledTemplate>>,
    pdf_backend: PdfBackend,
    font_manager: FontManager,
    debug: bool,
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        let mut font_manager = FontManager::new();
        // Always load the internal fallback font for maximum reliability.
        font_manager.load_fallback_font();
        Self {
            compiled_template: None,
            pdf_backend: Default::default(),
            font_manager,
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
        let extension = path_ref.extension().and_then(|s| s.to_str()).unwrap_or("");
        let resource_base_path = path_ref.parent().unwrap_or_else(|| Path::new("")).to_path_buf();

        let template_source = fs::read_to_string(path_ref).map_err(|e| {
            PipelineError::Io(io::Error::new(
                e.kind(),
                format!("Failed to read template from '{}': {}", path_ref.display(), e),
            ))
        })?;

        let parser: Box<dyn TemplateParser> = match extension {
            "xslt" | "xsl" | "fo" => Box::new(XsltParser),
            "json" => Box::new(JsonParser),
            _ => {
                return Err(PipelineError::Config(format!(
                    "Unsupported template file extension: .{}",
                    extension
                )))
            }
        };

        self.compiled_template = Some(parser.parse(&template_source, resource_base_path)?);
        Ok(self)
    }

    /// Configures the pipeline with a programmatically-built `Template` object.
    pub fn with_template_object(mut self, template: Template) -> Result<Self, PipelineError> {
        let template_source = template.to_json().map_err(|e| {
            PipelineError::Config(format!("Failed to serialize template object to JSON: {}", e))
        })?;

        let parser = JsonParser;
        let resource_base_path = PathBuf::new();
        self.compiled_template = Some(parser.parse(&template_source, resource_base_path)?);
        Ok(self)
    }

    /// Scans the host system for installed fonts and adds them to the pipeline's font database.
    /// This is the recommended way to get broad font support.
    pub fn with_system_fonts(mut self) -> Self {
        self.font_manager.load_system_fonts();
        self
    }

    /// Scans a directory for font files (`.ttf`, `.otf`, etc.) and adds them to the font database.
    /// Call this for any custom fonts not installed on the system.
    pub fn with_font_dir<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.font_manager.load_fonts_from_dir(path.as_ref());
        self
    }

    /// Selects the PDF rendering backend to use.
    pub fn with_pdf_backend(mut self, backend: PdfBackend) -> Self {
        self.pdf_backend = backend;
        self
    }

    /// Enables debug features, such as dumping the layout IR tree.
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Consumes the builder and creates the `DocumentPipeline`.
    pub fn build(mut self) -> Result<DocumentPipeline, PipelineError> {
        let compiled_template = self.compiled_template.take().ok_or_else(|| {
            PipelineError::Config(
                "No template has been configured. Use `with_template_file` or `with_template_object`."
                    .to_string(),
            )
        })?;

        Ok(DocumentPipeline::new(
            compiled_template,
            self.pdf_backend,
            Arc::new(self.font_manager),
            self.debug,
        ))
    }
}