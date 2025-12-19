use petty_core::error::PipelineError;
use crate::pipeline::api::PreparedDataSources;
use crate::pipeline::context::PipelineContext;
use serde_json::Value;

pub mod metadata;
pub mod passthrough;

use metadata::MetadataGeneratingProvider;
use passthrough::PassThroughProvider;

/// An enum for static dispatch of `DataSourceProvider` implementations.
/// This avoids the need for `Box<dyn ...>` which is not possible with the
/// generic `provide` method.
#[derive(Clone)]
pub enum Provider {
    PassThrough(PassThroughProvider),
    Metadata(MetadataGeneratingProvider),
}

impl DataSourceProvider for Provider {
    fn provide<'a, I>(
        &self,
        context: &'a PipelineContext,
        data_iterator: I,
    ) -> Result<PreparedDataSources, PipelineError>
    where
        I: Iterator<Item = Value> + Send + 'static,
    {
        match self {
            Provider::PassThrough(p) => p.provide(context, data_iterator),
            Provider::Metadata(p) => p.provide(context, data_iterator),
        }
    }
}

/// A trait for components that prepare data sources for the rendering stage.
///
/// A provider might be a simple pass-through that just boxes an iterator,
/// or it could be a complex component that consumes the entire data source
/// to perform an analysis pass, generating a `Document` object and a
/// temporary file for the main body content.
pub trait DataSourceProvider {
    /// Prepares the data sources for rendering.
    fn provide<'a, I>(
        &self,
        context: &'a PipelineContext,
        data_iterator: I,
    ) -> Result<PreparedDataSources, PipelineError>
    where
        I: Iterator<Item = Value> + Send + 'static;
}