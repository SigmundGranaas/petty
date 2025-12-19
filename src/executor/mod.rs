//! Executor implementations for the petty pipeline.
//!
//! This module contains platform-specific implementations of the
//! `Executor` trait from petty-core.
//!
//! ## Available Executors
//!
//! - [`RayonExecutor`]: Work-stealing thread pool (feature: `rayon-executor`)
//!
//! ## Re-exports
//!
//! For convenience, we also re-export the sync executor from petty-core:
//! - [`SyncExecutor`]: Sequential execution, no threading

#[cfg(feature = "rayon-executor")]
mod rayon;

#[cfg(feature = "rayon-executor")]
pub use self::rayon::RayonExecutor;

// Re-export the sync executor from petty-core for convenience
pub use petty_core::traits::SyncExecutor;

// Re-export the Executor trait for convenience
pub use petty_core::traits::Executor;

/// A type-erased executor that wraps concrete executor implementations.
///
/// Since the `Executor` trait has generic methods, it cannot be used as a trait object
/// (`dyn Executor`). This enum provides a workaround by holding concrete executor types
/// and delegating method calls to them.
///
/// This allows `PipelineContext` to store an executor without being generic over its type.
#[derive(Clone, Debug)]
pub enum ExecutorImpl {
    /// Sequential executor (no parallelism)
    Sync(SyncExecutor),

    /// Rayon work-stealing thread pool executor
    #[cfg(feature = "rayon-executor")]
    Rayon(RayonExecutor),
}

impl Executor for ExecutorImpl {
    fn execute_all<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Send + Sync + Clone + 'static,
    {
        match self {
            ExecutorImpl::Sync(exec) => exec.execute_all(items, f),
            #[cfg(feature = "rayon-executor")]
            ExecutorImpl::Rayon(exec) => exec.execute_all(items, f),
        }
    }

    fn execute_all_fallible<T, R, E, F>(&self, items: Vec<T>, f: F) -> Vec<Result<R, E>>
    where
        T: Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
        F: Fn(T) -> Result<R, E> + Send + Sync + Clone + 'static,
    {
        match self {
            ExecutorImpl::Sync(exec) => exec.execute_all_fallible(items, f),
            #[cfg(feature = "rayon-executor")]
            ExecutorImpl::Rayon(exec) => exec.execute_all_fallible(items, f),
        }
    }

    fn parallelism(&self) -> usize {
        match self {
            ExecutorImpl::Sync(exec) => exec.parallelism(),
            #[cfg(feature = "rayon-executor")]
            ExecutorImpl::Rayon(exec) => exec.parallelism(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ExecutorImpl::Sync(exec) => exec.name(),
            #[cfg(feature = "rayon-executor")]
            ExecutorImpl::Rayon(exec) => exec.name(),
        }
    }
}
