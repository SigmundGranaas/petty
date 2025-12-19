//! Resource providers for the Petty PDF pipeline.
//!
//! This crate provides platform-specific implementations of the
//! `ResourceProvider` trait from petty-traits.
//!
//! ## Available Providers
//!
//! - [`FilesystemResourceProvider`]: Loads resources from the local filesystem
//!
//! ## Re-exports
//!
//! For convenience, we also re-export the in-memory provider from petty-traits:
//! - [`InMemoryResourceProvider`]: Pre-populated in-memory storage

mod filesystem;

pub use filesystem::FilesystemResourceProvider;

// Re-export the in-memory provider from petty-traits for convenience
pub use petty_traits::InMemoryResourceProvider;
