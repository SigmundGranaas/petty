//! Core abstractions for template parsing and compilation
//!
//! This crate provides the foundational traits and types for template engines
//! in the Petty PDF system. It defines the contract between template parsers
//! (XSLT, JSON) and the execution/rendering pipeline.
//!
//! ## Key Abstractions
//!
//! - **`TemplateParser`**: Trait for parsing template source into compiled artifacts
//! - **`CompiledTemplate`**: Trait for executable template artifacts
//! - **`TemplateFeatures`**: Bundle of templates with feature detection
//! - **`TemplateFlags`**: Feature flags detected during compilation
//! - **`ExecutionConfig`**: Configuration for template execution

use petty_idf::IRNode;
use petty_style::stylesheet::Stylesheet;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during template processing
#[derive(Error, Debug)]
pub enum TemplateError {
    #[error("Template parsing failed: {0}")]
    ParseError(String),

    #[error("Template execution failed: {0}")]
    ExecutionError(String),

    #[error("Invalid template configuration: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Specifies the format of the input data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum DataSourceFormat {
    #[default]
    Xml,
    Json,
}


/// Configuration for an execution run.
#[derive(Debug, Clone, Default)]
pub struct ExecutionConfig {
    /// The format of the data source string.
    pub format: DataSourceFormat,
    /// If true, enables strict compliance checks.
    pub strict: bool,
}

/// A struct to report features found in a single template fragment.
#[derive(Debug, Default, Clone, Copy)]
pub struct TemplateFlags {
    /// True if the template contains a `<toc>` element or equivalent.
    pub has_table_of_contents: bool,
    /// True if the template contains a "page X of Y" placeholder.
    pub has_page_number_placeholders: bool,
    /// True if the template uses the `petty:index()` extension function.
    pub uses_index_function: bool,
    /// True if the template contains internal links (`<fo:link>`) or anchors (`id` attributes).
    pub has_internal_links: bool,
}

/// A bundle of all compiled templates and their collective features, returned by the parser.
pub struct TemplateFeatures {
    /// The main document template.
    pub main_template: Arc<dyn CompiledTemplate>,
    /// A map of templates for specific document roles (e.g., "page-header").
    pub role_templates: HashMap<String, Arc<dyn CompiledTemplate>>,
}

impl fmt::Debug for TemplateFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TemplateFeatures")
            .field("main_template", &"Arc<dyn CompiledTemplate>")
            .field(
                "role_templates",
                &self.role_templates.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// A dummy template that does nothing. It is used to provide a default
/// value for TemplateFeatures, which is required when deriving `Default` on
/// structs that contain it.
#[derive(Debug)]
struct NopTemplate;

impl CompiledTemplate for NopTemplate {
    fn execute(
        &self,
        _data_source: &str,
        _config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, TemplateError> {
        Ok(vec![])
    }

    fn stylesheet(&self) -> Arc<Stylesheet> {
        Arc::new(Stylesheet::default())
    }

    fn resource_base_path(&self) -> &Path {
        Path::new("")
    }

    fn features(&self) -> TemplateFlags {
        TemplateFlags::default()
    }
}

impl Default for TemplateFeatures {
    fn default() -> Self {
        Self {
            main_template: Arc::new(NopTemplate),
            role_templates: HashMap::new(),
        }
    }
}

impl TemplateFeatures {
    /// Checks if any template in the bundle uses the `petty:index()` extension function.
    pub fn uses_index_function(&self) -> bool {
        self.main_template.features().uses_index_function
            || self
                .role_templates
                .values()
                .any(|t| t.features().uses_index_function)
    }

    /// Checks if any template in the bundle contains a table of contents placeholder.
    pub fn has_table_of_contents(&self) -> bool {
        self.main_template.features().has_table_of_contents
            || self
                .role_templates
                .values()
                .any(|t| t.features().has_table_of_contents)
    }

    /// Checks if any template in the bundle contains page number placeholders.
    pub fn has_page_number_placeholders(&self) -> bool {
        self.main_template.features().has_page_number_placeholders
            || self
                .role_templates
                .values()
                .any(|t| t.features().has_page_number_placeholders)
    }

    /// Checks if any role-specific templates were defined.
    pub fn has_role_templates(&self) -> bool {
        !self.role_templates.is_empty()
    }

    /// A high-level check to see if the template requires any advanced, multi-pass processing.
    pub fn has_dependencies(&self) -> bool {
        self.has_table_of_contents()
            || self.has_page_number_placeholders()
            || self.has_role_templates()
            || self.uses_index_function()
    }
}

/// A reusable, data-agnostic, compiled template artifact.
pub trait CompiledTemplate: Send + Sync {
    /// Executes the template against a data context to produce a self-contained IRNode tree.
    fn execute(
        &self,
        data_source: &str,
        config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, TemplateError>;

    /// Returns a shared pointer to the stylesheet.
    fn stylesheet(&self) -> Arc<Stylesheet>;

    /// Returns the base path for resolving relative resource paths.
    fn resource_base_path(&self) -> &Path;

    /// Returns a summary of features detected in this specific template fragment.
    fn features(&self) -> TemplateFlags;
}

/// A parser responsible for compiling a template string into a `CompiledTemplate`.
pub trait TemplateParser {
    /// Parses a template source string.
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, TemplateError>;
}
