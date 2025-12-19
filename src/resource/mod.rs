//! Resource providers for the petty pipeline.
//!
//! This module contains platform-specific implementations of the
//! `ResourceProvider` trait from petty-core.
//!
//! ## Available Providers
//!
//! - [`FilesystemResourceProvider`]: Loads resources from the local filesystem
//!
//! ## Re-exports
//!
//! For convenience, we also re-export the in-memory provider from petty-core:
//! - [`InMemoryResourceProvider`]: Pre-populated in-memory storage

mod filesystem;

pub use filesystem::FilesystemResourceProvider;

// Re-export the in-memory provider from petty-core for convenience
pub use petty_core::traits::InMemoryResourceProvider;
