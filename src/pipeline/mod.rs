//! The main public API and orchestrator for document generation.

pub mod strategy;

mod builder;
mod config;
mod orchestrator;
pub(crate) mod worker;

pub use builder::PipelineBuilder;
pub use config::PdfBackend;