//! Executor implementations for the Petty PDF pipeline.
//!
//! This crate provides parallel execution strategies for processing
//! PDF generation workloads.
//!
//! ## Available Executors
//!
//! - [`RayonExecutor`]: Work-stealing thread pool (feature: `rayon`)
//! - [`SyncExecutor`]: Sequential execution (re-exported from petty-traits)
//!
//! ## Usage
//!
//! ```ignore
//! use petty_executor::{ExecutorImpl, RayonExecutor};
//! use petty_traits::Executor;
//!
//! let executor = ExecutorImpl::Rayon(RayonExecutor::new());
//! let results = executor.execute_all(vec![1, 2, 3], |x| x * 2);
//! ```

#[cfg(feature = "rayon")]
mod rayon_executor;

#[cfg(feature = "rayon")]
pub use rayon_executor::RayonExecutor;

// Re-export from petty-traits
pub use petty_traits::{Executor, SyncExecutor};

/// A type-erased executor that wraps concrete executor implementations.
///
/// Since the `Executor` trait has generic methods, it cannot be used as a trait object
/// (`dyn Executor`). This enum provides a workaround by holding concrete executor types
/// and delegating method calls to them.
#[derive(Clone, Debug)]
pub enum ExecutorImpl {
    /// Sequential executor (no parallelism)
    Sync(SyncExecutor),

    /// Rayon work-stealing thread pool executor
    #[cfg(feature = "rayon")]
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
            #[cfg(feature = "rayon")]
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
            #[cfg(feature = "rayon")]
            ExecutorImpl::Rayon(exec) => exec.execute_all_fallible(items, f),
        }
    }

    fn parallelism(&self) -> usize {
        match self {
            ExecutorImpl::Sync(exec) => exec.parallelism(),
            #[cfg(feature = "rayon")]
            ExecutorImpl::Rayon(exec) => exec.parallelism(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ExecutorImpl::Sync(exec) => exec.name(),
            #[cfg(feature = "rayon")]
            ExecutorImpl::Rayon(exec) => exec.name(),
        }
    }
}

impl Default for ExecutorImpl {
    fn default() -> Self {
        #[cfg(feature = "rayon")]
        {
            ExecutorImpl::Rayon(RayonExecutor::new())
        }
        #[cfg(not(feature = "rayon"))]
        {
            ExecutorImpl::Sync(SyncExecutor::new())
        }
    }
}
