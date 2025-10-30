// src/pipeline/mod.rs

//! The main public API and orchestrator for document generation.

mod builder;
mod config;
mod orchestrator;
pub(crate) mod worker;

pub use builder::PipelineBuilder;
pub use config::PdfBackend;