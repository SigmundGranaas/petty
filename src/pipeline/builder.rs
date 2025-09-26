use super::config::{JsonTemplate, PdfBackend, Template, XsltTemplate};
use super::orchestrator::DocumentPipeline;
use crate::error::PipelineError;
use crate::parser::json::ast::StylesheetDef;
use crate::parser::xslt::compiler::Compiler;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path};
use std::sync::Arc;
use crate::core::layout::FontManager;
use crate::core::style::stylesheet::Stylesheet;

/// A builder for creating a `DocumentPipeline`.
#[derive(Default)]
pub struct PipelineBuilder {
    stylesheet: Option<Stylesheet>,
    template: Option<Template>,
    pdf_backend: PdfBackend,
    font_manager: Option<Arc<FontManager>>,
}

impl PipelineBuilder {
    /// Creates a new, empty `PipelineBuilder`.
    pub fn new() -> Self {
        Default::default()
    }

    /// Configures the pipeline to use an XSLT template from a file.
    /// This also extracts stylesheet information and loads fonts from the same directory.
    pub fn with_xslt_template_file<P: AsRef<Path>>(
        self,
        path: P,
    ) -> Result<Self, PipelineError> {
        let path_ref = path.as_ref();
        let xslt_content = fs::read_to_string(path_ref).map_err(|e| {
            PipelineError::Io(io::Error::new(
                e.kind(),
                format!(
                    "Failed to read XSLT template from '{}': {}",
                    path_ref.display(),
                    e
                ),
            ))
        })?;
        let base_path = path_ref.parent();

        let font_manager = Self::create_font_manager(base_path)?;
        let mut s = self.with_xslt_template_str(&xslt_content, base_path)?;
        s.font_manager = Some(Arc::new(font_manager));
        Ok(s)
    }

    /// Configures the pipeline to use an XSLT template from a string.
    pub fn with_xslt_template_str(
        self,
        xslt_content: &str,
        resource_base_path: Option<&Path>,
    ) -> Result<Self, PipelineError> {
        let compiled_stylesheet = Compiler::compile(xslt_content).map_err(PipelineError::from)?;
        let mut stylesheet = Stylesheet::from_xslt(xslt_content)?;
        stylesheet.styles = compiled_stylesheet.styles.clone();
        let base_path = resource_base_path
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();

        Ok(Self {
            stylesheet: Some(stylesheet),
            template: Some(Template::Xslt(XsltTemplate {
                compiled_stylesheet,
                resource_base_path: base_path,
            })),
            ..self
        })
    }

    /// Configures the pipeline to use a JSON template from a file.
    pub fn with_json_template_file<P: AsRef<Path>>(
        self,
        path: P,
    ) -> Result<Self, PipelineError> {
        let path_ref = path.as_ref();
        let json_content = fs::read_to_string(path_ref).map_err(|e| {
            PipelineError::Io(io::Error::new(
                e.kind(),
                format!(
                    "Failed to read JSON template from '{}': {}",
                    path_ref.display(),
                    e
                ),
            ))
        })?;
        let base_path = path_ref.parent();

        let font_manager = Self::create_font_manager(base_path)?;
        let mut s = self.with_json_template_str(&json_content, base_path)?;
        s.font_manager = Some(Arc::new(font_manager));
        Ok(s)
    }

    /// Configures the pipeline to use a JSON template from a string.
    pub fn with_json_template_str(
        self,
        json_content: &str,
        resource_base_path: Option<&Path>,
    ) -> Result<Self, PipelineError> {
        // 1. Parse the entire template into a generic Value to safely extract parts.
        let template_value: Value =
            serde_json::from_str(json_content).map_err(crate::parser::ParseError::JsonParse)?;

        // 2. Extract and deserialize the stylesheet part. This part must NOT contain templates.
        let stylesheet_value = template_value.get("_stylesheet").ok_or_else(|| {
            PipelineError::StylesheetError("JSON template missing `_stylesheet` key.".into())
        })?;

        let stylesheet_def: StylesheetDef = serde_json::from_value(stylesheet_value.clone())
            .map_err(|e| {
                PipelineError::StylesheetError(format!("Failed to parse `_stylesheet` block: {}", e))
            })?;

        let stylesheet: Stylesheet = stylesheet_def.into();
        let base_path = resource_base_path
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();

        Ok(Self {
            stylesheet: Some(stylesheet),
            template: Some(Template::Json(JsonTemplate {
                template_content: Arc::new(json_content.to_string()),
                resource_base_path: base_path,
            })),
            ..self
        })
    }

    /// Selects the PDF rendering backend to use.
    pub fn with_pdf_backend(self, backend: PdfBackend) -> Self {
        Self {
            pdf_backend: backend,
            ..self
        }
    }

    /// Consumes the builder and creates the `DocumentPipeline`.
    pub fn build(mut self) -> Result<DocumentPipeline, PipelineError> {
        let stylesheet = self.stylesheet.ok_or_else(|| {
            PipelineError::StylesheetError("No stylesheet or template provided".to_string())
        })?;
        let template = self.template.ok_or_else(|| {
            PipelineError::StylesheetError("Template language could not be determined".to_string())
        })?;

        let font_manager = self.font_manager.take().unwrap_or_else(|| {
            let mut fm = FontManager::new();
            fm.load_fallback_font().expect("Failed to load fallback font");
            Arc::new(fm)
        });

        let generator = DocumentPipeline::new(stylesheet, template, self.pdf_backend, font_manager);
        Ok(generator)
    }

    /// Helper to find and load fonts from standard directories.
    fn create_font_manager(base_path: Option<&Path>) -> Result<FontManager, PipelineError> {
        let mut font_manager = FontManager::new();
        let font_path = Path::new("assets/fonts");
        if font_path.exists() && font_path.is_dir() {
            font_manager.load_fonts_from_dir(font_path)?;
        } else if let Some(bp) = base_path {
            let relative_font_path = bp.join("fonts");
            if relative_font_path.exists() && relative_font_path.is_dir() {
                font_manager.load_fonts_from_dir(&relative_font_path)?;
            }
        }
        font_manager.load_fallback_font()?;
        Ok(font_manager)
    }
}