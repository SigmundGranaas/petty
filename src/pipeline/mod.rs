//! The main public API and orchestrator for document generation.

mod builder;
mod config;
mod orchestrator;
pub mod api;
pub mod provider;
pub mod renderer;
pub(crate) mod concurrency;
pub(crate) mod context;
pub(crate) mod worker;


pub use builder::PipelineBuilder;
pub use config::PdfBackend;