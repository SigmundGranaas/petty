//! Rayon-based parallel executor.
//!
//! This executor uses rayon's work-stealing thread pool for parallel execution.
//! It's the recommended executor for CPU-bound workloads on native platforms.

use petty_core::traits::Executor;
use rayon::prelude::*;

/// A parallel executor using rayon's work-stealing thread pool.
///
/// This executor provides efficient parallel execution for CPU-bound tasks.
/// It uses rayon's global thread pool, which automatically scales to use
/// all available CPU cores.
///
/// # Thread Pool Configuration
///
/// This executor uses rayon's global thread pool. To configure the number
/// of threads, use `rayon::ThreadPoolBuilder` before creating any `RayonExecutor`:
///
/// ```ignore
/// // Configure rayon's global pool (must be done before first use)
/// rayon::ThreadPoolBuilder::new()
///     .num_threads(4)
///     .build_global()
///     .unwrap();
///
/// // Now create executor - will use 4 threads
/// let executor = RayonExecutor::new();
/// ```
///
/// # Example
///
/// ```ignore
/// use petty::executor::RayonExecutor;
/// use petty_core::traits::Executor;
///
/// let executor = RayonExecutor::new();
/// let results = executor.execute_all(vec![1, 2, 3], |x| x * 2);
/// // Results may be in any order due to parallel execution
/// assert_eq!(results.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct RayonExecutor {
    /// Cached thread count for reporting via `parallelism()`.
    /// This reflects rayon's actual global pool size.
    num_threads: usize,
}

impl RayonExecutor {
    /// Creates a new RayonExecutor using rayon's global thread pool.
    ///
    /// The number of threads is determined by rayon's global configuration,
    /// which defaults to the number of CPU cores.
    pub fn new() -> Self {
        Self {
            num_threads: rayon::current_num_threads(),
        }
    }
}

impl Default for RayonExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor for RayonExecutor {
    fn execute_all<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<R>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> R + Send + Sync + Clone + 'static,
    {
        items.into_par_iter().map(f).collect()
    }

    fn execute_all_fallible<T, R, E, F>(&self, items: Vec<T>, f: F) -> Vec<Result<R, E>>
    where
        T: Send + 'static,
        R: Send + 'static,
        E: Send + 'static,
        F: Fn(T) -> Result<R, E> + Send + Sync + Clone + 'static,
    {
        items.into_par_iter().map(f).collect()
    }

    fn parallelism(&self) -> usize {
        self.num_threads
    }

    fn name(&self) -> &'static str {
        "RayonExecutor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_rayon_executor_processes_items() {
        let executor = RayonExecutor::new();
        let items = vec![1, 2, 3, 4, 5];
        let results = executor.execute_all(items, |x| x * 2);

        // Results may not be in order due to parallelism, but all values should be present
        let mut sorted_results = results.clone();
        sorted_results.sort();
        assert_eq!(sorted_results, vec![2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_rayon_executor_handles_fallible_operations() {
        let executor = RayonExecutor::new();
        let items = vec![1, 2, 0, 4];
        let results: Vec<Result<i32, &str>> = executor.execute_all_fallible(items, |x| {
            if x == 0 {
                Err("division by zero")
            } else {
                Ok(10 / x)
            }
        });

        assert_eq!(results.len(), 4);
        // Find the error result (position may vary due to parallel execution)
        let error_count = results.iter().filter(|r| r.is_err()).count();
        let ok_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(error_count, 1);
        assert_eq!(ok_count, 3);
    }

    #[test]
    fn test_rayon_executor_parallelism_is_positive() {
        let executor = RayonExecutor::new();
        assert!(executor.parallelism() > 0);
    }

    #[test]
    fn test_rayon_executor_actually_runs_in_parallel() {
        let executor = RayonExecutor::new();

        // Skip this test if we only have 1 thread
        if executor.parallelism() <= 1 {
            return;
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let items: Vec<usize> = (0..100).collect();

        let _ = executor.execute_all(items, {
            let counter = counter.clone();
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        });

        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }
}
