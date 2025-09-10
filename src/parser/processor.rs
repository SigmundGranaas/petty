// src/parser/processor.rs
use crate::error::PipelineError;
use crate::idf::IDFEvent;
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;

/// A proxy that allows a parser to send events into the async pipeline channel.
pub struct LayoutProcessorProxy<'a> {
    tx: mpsc::Sender<IDFEvent<'a>>,
}

impl<'a> LayoutProcessorProxy<'a> {
    pub fn new(tx: mpsc::Sender<IDFEvent<'a>>) -> Self {
        Self { tx }
    }
    /// Asynchronously sends an event into the pipeline.
    pub async fn process_event(&mut self, event: IDFEvent<'a>) -> Result<(), PipelineError> {
        self.tx
            .send(event)
            .await
            .map_err(|e| PipelineError::TemplateParseError(format!("Channel send error: {}", e)))
    }
}

/// A trait for processing a template and data into a stream of layout events.
/// This allows for multiple templating languages (e.g., JSON, XSLT) to be
/// used interchangeably by the document generation pipeline.
#[async_trait(?Send)]
pub trait TemplateProcessor<'a> {
    /// Asynchronously processes the given data with its configured template, feeding `IDFEvent`s
    /// to the layout processor via the proxy.
    ///
    /// # Arguments
    /// * `data` - The source `serde_json::Value` to use for templating.
    /// * `proxy` - The proxy to send events into the pipeline.
    async fn process(
        &mut self,
        data: &'a Value,
        proxy: &mut LayoutProcessorProxy<'a>,
    ) -> Result<(), PipelineError>;
}