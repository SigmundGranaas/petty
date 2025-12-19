//! Platform abstraction traits for petty-core.
//!
//! This module contains traits that abstract platform-specific functionality,
//! allowing petty-core to run on any platform including WASM.
//!
//! ## Traits
//!
//! - [`Executor`] - Abstracts parallel execution (single-threaded, rayon, tokio)
//! - [`ResourceProvider`] - Abstracts resource loading (filesystem, in-memory, remote)
//! - [`FontProvider`] - Abstracts font discovery and loading
//!
//! ## Default Implementations
//!
//! Each trait comes with at least one platform-agnostic implementation:
//!
//! - [`SyncExecutor`] - Sequential execution, no threading
//! - [`InMemoryResourceProvider`] - Pre-populated in-memory resource storage
//! - [`InMemoryFontProvider`] - Pre-populated in-memory font storage
//!
//! These implementations work in any environment including WASM.
//!
//! ## Platform-Specific Implementations
//!
//! The `petty` crate provides additional implementations for native platforms:
//!
//! - `RayonExecutor` - Work-stealing thread pool via rayon
//! - `TokioExecutor` - Async runtime via tokio
//! - `FilesystemResourceProvider` - Load resources from filesystem
//! - `SystemFontProvider` - Discover and load system fonts

mod executor;
mod font;
mod resource;

// Re-export traits
pub use executor::{Executor, ExecutorError, SyncExecutor};
pub use font::{FontDescriptor, FontError, FontProvider, FontQuery, InMemoryFontProvider, SharedFontData};
pub use resource::{InMemoryResourceProvider, ResourceError, ResourceProvider, SharedResourceData};
