//! Executor trait for abstracting parallel execution.
//!
//! This trait allows the layout engine to perform parallel work without
//! being tied to a specific threading implementation (tokio, rayon, etc.).

use std::fmt::Debug;

/// Error type for executor operations.
#[derive(Debug, Clone)]
pub struct ExecutorError {
    pub message: String,
}

impl std::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Executor error: {}", self.message)
    }
}

impl std::error::Error for ExecutorError {}

impl ExecutorError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

/// A trait for executing work items, potentially in parallel.
///
/// This abstraction allows the layout engine to leverage parallelism
/// without being tied to a specific runtime (tokio, rayon, std::thread, etc.).
///
/// # Implementations
///
/// - `SyncExecutor`: Sequential execution, no threading (always available)
/// - `RayonExecutor`: Work-stealing thread pool (feature-gated)
/// - `TokioExecutor`: Async runtime with spawn_blocking (feature-gated)
///
/// # Example
///
/// ```ignore
/// let executor: Box<dyn Executor> = Box::new(SyncExecutor::new());
/// let results = executor.execute_all(items, |item| process(item));
/// ```
pub trait Executor: Send + Sync + Debug {
    /// Execute a batch of work items, potentially in parallel.
    ///
    /// The function `f` is called for each item. Results are returned
    /// in the same order as the input items.
    ///
    /// # Arguments
    ///
    /// * `items` - The work items to process
    /// * `f` - The function to apply to each item
    ///
    /// # Returns
    ///
    /// A vector of results in the same order as the input items.
    fn execute_all<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Send + Sync + Clone + 'static;

    /// Execute a batch of work items that may fail.
    ///
    /// Similar to `execute_all` but handles fallible operations.
    fn execute_all_fallible<T, R, E, F>(&self, items: Vec<T>, f: F) -> Vec<Result<R, E>>
    where
        T: Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
        F: Fn(T) -> Result<R, E> + Send + Sync + Clone + 'static;

    /// Returns the level of parallelism this executor can provide.
    ///
    /// - Returns 1 for sequential executors
    /// - Returns CPU count or configured thread count for parallel executors
    fn parallelism(&self) -> usize;

    /// Returns a human-readable name for this executor (for logging/debugging).
    fn name(&self) -> &'static str;
}

/// A synchronous executor that processes items sequentially.
///
/// This is the simplest executor implementation with no threading overhead.
/// It's always available and works in any environment including WASM.
#[derive(Debug, Clone, Default)]
pub struct SyncExecutor;

impl SyncExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Executor for SyncExecutor {
    fn execute_all<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Send + Sync + Clone + 'static,
    {
        items.into_iter().map(f).collect()
    }

    fn execute_all_fallible<T, R, E, F>(&self, items: Vec<T>, f: F) -> Vec<Result<R, E>>
    where
        T: Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
        F: Fn(T) -> Result<R, E> + Send + Sync + Clone + 'static,
    {
        items.into_iter().map(f).collect()
    }

    fn parallelism(&self) -> usize {
        1
    }

    fn name(&self) -> &'static str {
        "SyncExecutor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_executor_processes_items_in_order() {
        let executor = SyncExecutor::new();
        let items = vec![1, 2, 3, 4, 5];
        let results = executor.execute_all(items, |x| x * 2);
        assert_eq!(results, vec![2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_sync_executor_handles_fallible_operations() {
        let executor = SyncExecutor::new();
        let items = vec![1, 2, 0, 4];
        let results: Vec<Result<i32, &str>> = executor.execute_all_fallible(items, |x| {
            if x == 0 {
                Err("division by zero")
            } else {
                Ok(10 / x)
            }
        });
        assert_eq!(results.len(), 4);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_err());
        assert!(results[3].is_ok());
    }

    #[test]
    fn test_sync_executor_parallelism_is_one() {
        let executor = SyncExecutor::new();
        assert_eq!(executor.parallelism(), 1);
    }

    // Edge case tests

    #[test]
    fn test_sync_executor_empty_input() {
        let executor = SyncExecutor::new();
        let items: Vec<i32> = vec![];
        let results = executor.execute_all(items, |x| x * 2);
        assert!(results.is_empty());
    }

    #[test]
    fn test_sync_executor_empty_fallible_input() {
        let executor = SyncExecutor::new();
        let items: Vec<i32> = vec![];
        let results: Vec<Result<i32, &str>> = executor.execute_all_fallible(items, |x| Ok(x));
        assert!(results.is_empty());
    }

    #[test]
    fn test_sync_executor_single_item() {
        let executor = SyncExecutor::new();
        let items = vec![42];
        let results = executor.execute_all(items, |x| x * 2);
        assert_eq!(results, vec![84]);
    }

    #[test]
    fn test_sync_executor_all_failures() {
        let executor = SyncExecutor::new();
        let items = vec![0, 0, 0];
        let results: Vec<Result<i32, &str>> = executor.execute_all_fallible(items, |_| {
            Err("always fails")
        });
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_err()));
    }

    #[test]
    fn test_sync_executor_name() {
        let executor = SyncExecutor::new();
        assert_eq!(executor.name(), "SyncExecutor");
    }

    #[test]
    fn test_executor_error_display() {
        let err = ExecutorError::new("test error");
        assert_eq!(err.to_string(), "Executor error: test error");
    }
}
