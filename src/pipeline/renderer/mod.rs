// src/pipeline/renderer/mod.rs
//!
//! Document rendering strategies.
//!
//! This module provides two rendering approaches:
//!
//! - [`SinglePassStreamingRenderer`]: High-performance streaming renderer with
//!   true pipeline parallelism. Used for simple templates without forward references.
//!
//! - [`ComposingRenderer`]: Two-pass renderer for templates with forward references
//!   (ToC, page numbers, internal links). Renders body first, then composes with metadata.

use crate::pipeline::api::PreparedDataSources;
use crate::pipeline::context::PipelineContext;
use crate::pipeline::renderer::composing::ComposingRenderer;
use crate::pipeline::renderer::streaming::SinglePassStreamingRenderer;
use petty_core::error::PipelineError;
use std::io::{Seek, Write};

pub mod composing;
pub mod streaming;

/// An enum for static dispatch of `RenderingStrategy` implementations.
///
/// This enum provides two variants:
/// - `Streaming`: High-performance single-pass rendering for simple templates
/// - `Composing`: Two-pass rendering for templates with forward references
#[derive(Clone)]
pub enum Renderer {
    /// Single-pass streaming renderer with pipeline parallelism.
    Streaming(SinglePassStreamingRenderer),
    /// Two-pass composing renderer for templates with forward references.
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
