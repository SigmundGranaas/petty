//! Template processor abstractions - re-exported from petty-template-core
//!
//! This module provides backward compatibility by re-exporting types from
//! petty-template-core and providing adapters for PipelineError.

use crate::error::PipelineError;
use crate::idf::IRNode;
use crate::style_types::stylesheet::Stylesheet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// Re-export core types from petty-template-core
pub use petty_template_core::{
    DataSourceFormat, ExecutionConfig, TemplateFeatures as CoreTemplateFeatures, TemplateFlags,
};

// Re-export TemplateError for use by parser implementations
pub use petty_template_core::TemplateError;

/// Backward-compatible wrapper for TemplateFeatures that works with PipelineError
pub struct TemplateFeatures {
    /// The main document template.
    pub main_template: Arc<dyn CompiledTemplate>,
    /// A map of templates for specific document roles (e.g., "page-header").
    pub role_templates: HashMap<String, Arc<dyn CompiledTemplate>>,
}

impl TemplateFeatures {
    /// Create from the core TemplateFeatures
    pub fn from_core(features: CoreTemplateFeatures) -> Self {
        // Wrap templates with adapters
        let main_template: Arc<dyn CompiledTemplate> =
            Arc::new(CompiledTemplateAdapter::new(features.main_template));

        let role_templates: HashMap<String, Arc<dyn CompiledTemplate>> = features
            .role_templates
            .into_iter()
            .map(|(name, template)| {
                let wrapped: Arc<dyn CompiledTemplate> =
                    Arc::new(CompiledTemplateAdapter::new(template));
                (name, wrapped)
            })
            .collect();

        Self {
            main_template,
            role_templates,
        }
    }

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

impl From<CoreTemplateFeatures> for TemplateFeatures {
    fn from(features: CoreTemplateFeatures) -> Self {
        Self::from_core(features)
    }
}

impl std::fmt::Debug for TemplateFeatures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplateFeatures")
            .field("main_template", &"Arc<dyn CompiledTemplate>")
            .field(
                "role_templates",
                &self.role_templates.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Adapter trait for CompiledTemplate that works with PipelineError
pub trait CompiledTemplate: Send + Sync {
    /// Executes the template against a data context to produce a self-contained IRNode tree.
    fn execute(
        &self,
        data_source: &str,
        config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, PipelineError>;

    /// Returns a shared pointer to the stylesheet.
    fn stylesheet(&self) -> Arc<Stylesheet>;

    /// Returns the base path for resolving relative resource paths.
    fn resource_base_path(&self) -> &Path;

    /// Returns a summary of features detected in this specific template fragment.
    fn features(&self) -> TemplateFlags;
}

/// Adapter for core CompiledTemplate to work with PipelineError
pub struct CompiledTemplateAdapter {
    inner: Arc<dyn petty_template_core::CompiledTemplate>,
}

impl CompiledTemplateAdapter {
    pub fn new(inner: Arc<dyn petty_template_core::CompiledTemplate>) -> Self {
        Self { inner }
    }
}

impl CompiledTemplate for CompiledTemplateAdapter {
    fn execute(
        &self,
        data_source: &str,
        config: ExecutionConfig,
    ) -> Result<Vec<IRNode>, PipelineError> {
        Ok(self.inner.execute(data_source, config)?)
    }

    fn stylesheet(&self) -> Arc<Stylesheet> {
        self.inner.stylesheet()
    }

    fn resource_base_path(&self) -> &Path {
        self.inner.resource_base_path()
    }

    fn features(&self) -> TemplateFlags {
        self.inner.features()
    }
}

/// A parser responsible for compiling a template string into a `CompiledTemplate`.
pub trait TemplateParser {
    /// Parses a template source string.
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, PipelineError>;
}

/// Adapter for core TemplateParser to work with PipelineError
pub struct TemplateParserAdapter<P> {
    inner: P,
}

impl<P> TemplateParserAdapter<P>
where
    P: petty_template_core::TemplateParser,
{
    pub fn new(parser: P) -> Self {
        Self { inner: parser }
    }
}

impl<P> TemplateParser for TemplateParserAdapter<P>
where
    P: petty_template_core::TemplateParser,
{
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, PipelineError> {
        Ok(TemplateFeatures::from_core(
            self.inner.parse(template_source, resource_base_path)?,
        ))
    }
}
