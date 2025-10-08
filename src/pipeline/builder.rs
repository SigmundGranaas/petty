use super::config::PdfBackend;
use super::orchestrator::DocumentPipeline;
use crate::core::layout::FontManager;
use crate::error::PipelineError;
use crate::parser::json::processor::JsonParser;
use crate::parser::processor::{CompiledTemplate, TemplateParser};
use crate::parser::xslt::processor::XsltParser;
use crate::templating::Template;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A builder for creating a `DocumentPipeline`.
#[derive(Default)]
pub struct PipelineBuilder {
    compiled_template: Option<Arc<dyn CompiledTemplate>>,
    pdf_backend: PdfBackend,
    font_manager: Option<Arc<FontManager>>,
    debug: bool,
}

impl PipelineBuilder {
    /// Creates a new, empty `PipelineBuilder`.
    pub fn new() -> Self {
        Default::default()
    }

    /// Configures the pipeline by loading a template from a file.
    /// The template language (XSLT, JSON) is inferred from the file extension.
    /// This also discovers and loads fonts from a `fonts` subdirectory relative to the template.
    pub fn with_template_file<P: AsRef<Path>>(mut self, path: P) -> Result<Self, PipelineError> {
        let path_ref = path.as_ref();
        let extension = path_ref.extension().and_then(|s| s.to_str()).unwrap_or("");
        let resource_base_path = path_ref.parent().unwrap_or_else(|| Path::new("")).to_path_buf();

        let template_source = fs::read_to_string(path_ref).map_err(|e| {
            PipelineError::Io(io::Error::new(e.kind(), format!("Failed to read template from '{}': {}", path_ref.display(), e)))
        })?;

        let parser: Box<dyn TemplateParser> = match extension {
            "xslt" | "xsl" | "fo" => Box::new(XsltParser),
            "json" => Box::new(JsonParser),
            _ => return Err(PipelineError::Config(format!("Unsupported template file extension: .{}", extension))),
        };

        self.compiled_template = Some(parser.parse(&template_source, resource_base_path)?);
        self.font_manager = Some(Arc::new(Self::create_font_manager(path_ref.parent())?));

        Ok(self)
    }

    /// Configures the pipeline with a programmatically-built `Template` object.
    /// This is the entry point for the code-based builder API.
    /// This also discovers and loads fonts from the `./assets/fonts` directory.
    pub fn with_template_object(mut self, template: Template) -> Result<Self, PipelineError> {
        let template_source = template.to_json().map_err(|e| {
            PipelineError::Config(format!("Failed to serialize template object to JSON: {}", e))
        })?;

        let parser = JsonParser;
        // For a template object, the resource base path is the current working directory.
        let resource_base_path = PathBuf::new();

        self.compiled_template = Some(parser.parse(&template_source, resource_base_path)?);
        // Since there's no file, we only search for fonts in standard locations like ./assets/fonts
        self.font_manager = Some(Arc::new(Self::create_font_manager(None)?));

        Ok(self)
    }

    /// Selects the PDF rendering backend to use.
    pub fn with_pdf_backend(self, backend: PdfBackend) -> Self {
        Self {
            pdf_backend: backend,
            ..self
        }
    }

    /// Enables debug features, such as dumping the layout IR tree.
    pub fn with_debug(self, debug: bool) -> Self {
        Self { debug, ..self }
    }

    /// Consumes the builder and creates the `DocumentPipeline`.
    pub fn build(mut self) -> Result<DocumentPipeline, PipelineError> {
        let compiled_template = self.compiled_template.take().ok_or_else(|| {
            PipelineError::Config("No template has been configured. Use `with_template_file` or `with_template_object`.".to_string())
        })?;

        let font_manager = self.font_manager.take().unwrap_or_else(|| {
            let mut fm = FontManager::new();
            fm.load_fallback_font().expect("Failed to load fallback font");
            Arc::new(fm)
        });

        Ok(DocumentPipeline::new(compiled_template, self.pdf_backend, font_manager, self.debug))
    }

    /// Helper to find and load fonts from standard directories.
    fn create_font_manager(base_path: Option<&Path>) -> Result<FontManager, PipelineError> {
        let mut font_manager = FontManager::new();
        // Check for `assets/fonts` in cwd
        let font_path = Path::new("assets/fonts");
        if font_path.exists() && font_path.is_dir() {
            font_manager.load_fonts_from_dir(font_path)?;
        }
        // Check for `fonts` relative to template
        if let Some(bp) = base_path {
            let relative_font_path = bp.join("fonts");
            if relative_font_path.exists() && relative_font_path.is_dir() {
                font_manager.load_fonts_from_dir(&relative_font_path)?;
            }
        }
        font_manager.load_fallback_font()?;
        Ok(font_manager)
    }
}