//! The main public API and orchestrator for document generation.

mod adapters;
pub mod api;
mod builder;
pub(crate) mod concurrency;
mod config;
pub(crate) mod context;
mod orchestrator;
pub mod provider;
pub mod renderer;
pub(crate) mod worker;

pub use builder::PipelineBuilder;
pub use config::{GenerationMode, PdfBackend};
