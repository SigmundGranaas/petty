// src/pipeline/renderer/mod.rs
use petty_core::error::PipelineError;
use crate::pipeline::api::PreparedDataSources;
use crate::pipeline::context::PipelineContext;
use crate::pipeline::renderer::composing::ComposingRenderer;
use crate::pipeline::renderer::streaming::SinglePassStreamingRenderer;
use std::io::{Seek, Write};

pub mod composing;
pub mod streaming;

/// An enum for static dispatch of `RenderingStrategy` implementations.
/// This avoids the need for `Box<dyn ...>` which is not possible with the
/// generic `render` method.
#[derive(Clone)]
pub enum Renderer {
    Streaming(SinglePassStreamingRenderer),
    Composing(ComposingRenderer),
}

impl RenderingStrategy for Renderer {
    fn render<W>(
        &self,
        context: &PipelineContext,
        sources: PreparedDataSources,
        writer: W,
    ) -> Result<W, PipelineError>
    where
        W: Write + Seek + Send + 'static,
    {
        match self {
            Renderer::Streaming(r) => r.render(context, sources, writer),
            Renderer::Composing(r) => r.render(context, sources, writer),
        }
    }
}

/// A trait for components that consume prepared data sources and render a final document.
///
/// A rendering strategy might be a simple streaming renderer that consumes the
/// data iterator directly, or it could be an advanced composing renderer that
//  merges a pre-rendered body with new content (like a ToC) generated from
//  the `Document` metadata object.
pub trait RenderingStrategy {
    /// Renders the final document to the provided writer.
    fn render<W>(
        &self,
        context: &PipelineContext,
        sources: PreparedDataSources,
        writer: W,
    ) -> Result<W, PipelineError>
    where
        W: Write + Seek + Send + 'static;
}