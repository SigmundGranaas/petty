//! Adaptive worker scaling for the PDF generation pipeline.
//!
//! This module provides runtime optimization based on throughput metrics,
//! automatically adjusting worker count to maximize CPU utilization.
//!
//! # Features
//!
//! - **AdaptiveController**: Thread-safe metrics collector with scaling recommendations
//! - **WorkerManager**: Coordinates worker spawning and cooperative shutdown (requires `adaptive-scaling` feature)
//! - **AdaptiveConfig**: Tunable parameters for scaling behavior
//!
//! # Feature Flags
//!
//! - `adaptive-scaling`: Enables dynamic worker scaling at runtime. Without this feature,
//!   only metrics collection is available.
//!
//! # Example
//!
//! ```ignore
//! use petty::pipeline::{AdaptiveController, AdaptiveConfig};
//! use std::sync::Arc;
//!
//! let config = AdaptiveConfig::default();
//! let controller = Arc::new(AdaptiveController::with_config(4, config));
//!
//! // Record metrics during processing (always available)
//! controller.record_item_processed(Duration::from_millis(50));
//! controller.record_queue_depth(10);
//!
//! // Check if scaling is recommended (requires adaptive-scaling feature)
//! #[cfg(feature = "adaptive-scaling")]
//! if controller.should_scale_up() {
//!     // Spawn additional worker
//! }
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Configuration for the adaptive controller.
///
/// All thresholds are relative to the current worker count.
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Minimum number of workers to maintain (default: 2)
    pub min_workers: usize,
    /// Maximum number of workers to spawn (default: num_cpus * 2)
    pub max_workers: usize,
    /// Scale up when queue depth exceeds workers * this value (default: 2.0)
    pub scale_up_threshold: f64,
    /// Scale down when queue depth falls below workers * this value (default: 0.25)
    pub scale_down_threshold: f64,
    /// Minimum time between scaling adjustments (default: 500ms)
    pub adjustment_cooldown: Duration,
    /// How often to check for scaling opportunities (every N items received).
    /// Only used when ProcessingMode::Adaptive is enabled. (default: 10)
    pub scaling_check_interval: usize,
    /// Extra headroom added to worker count for max in-flight items.
    /// Controls semaphore permits: max_in_flight = workers + this value. (default: 2)
    pub max_in_flight_buffer: usize,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            min_workers: 2,
            max_workers: num_cpus::get().saturating_mul(2),
            scale_up_threshold: 2.0,
            scale_down_threshold: 0.25,
            adjustment_cooldown: Duration::from_millis(500),
            scaling_check_interval: 10,
            max_in_flight_buffer: 2,
        }
    }
}

impl AdaptiveConfig {
    /// Create a configuration with custom worker bounds.
    pub fn with_worker_bounds(min: usize, max: usize) -> Self {
        Self {
            min_workers: min.max(1),
            max_workers: max.max(min),
            ..Default::default()
        }
    }
}

/// Thread-safe controller for adaptive worker scaling.
///
/// Tracks throughput metrics and determines when to scale workers up or down.
/// All operations use appropriate atomic orderings for cross-thread visibility.
pub struct AdaptiveController {
    /// Current number of active workers
    current_worker_count: AtomicUsize,
    /// Total items processed
    items_processed: AtomicUsize,
    /// Total processing time in nanoseconds
    total_processing_time_ns: AtomicU64,
    /// High water mark for queue depth
    queue_high_water: AtomicUsize,
    /// Current queue depth
    current_queue_depth: AtomicUsize,
    /// Last adjustment timestamp (nanos since start_time)
    last_adjustment_nanos: AtomicU64,
    /// Start time for throughput calculation
    start_time: Instant,
    /// Configuration
    config: AdaptiveConfig,
}

impl AdaptiveController {
    /// Create a new adaptive controller with the specified initial worker count.
    #[allow(dead_code)] // Public API - may not be used internally
    pub fn new(initial_workers: usize) -> Self {
        Self::with_config(initial_workers, AdaptiveConfig::default())
    }

    /// Create a new adaptive controller with custom configuration.
    pub fn with_config(initial_workers: usize, config: AdaptiveConfig) -> Self {
        let clamped_workers = initial_workers.clamp(config.min_workers, config.max_workers);
        Self {
            current_worker_count: AtomicUsize::new(clamped_workers),
            items_processed: AtomicUsize::new(0),
            total_processing_time_ns: AtomicU64::new(0),
            queue_high_water: AtomicUsize::new(0),
            current_queue_depth: AtomicUsize::new(0),
            last_adjustment_nanos: AtomicU64::new(0),
            start_time: Instant::now(),
            config,
        }
    }

    /// Record that an item was processed with the given duration.
    ///
    /// Uses Release ordering to ensure visibility to other threads.
    pub fn record_item_processed(&self, duration: Duration) {
        self.items_processed.fetch_add(1, Ordering::Release);
        // Saturating conversion to prevent overflow on very long durations
        let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        self.total_processing_time_ns
            .fetch_add(nanos, Ordering::Release);
    }

    /// Record the current queue depth.
    ///
    /// Also updates the high water mark if this is a new maximum.
    pub fn record_queue_depth(&self, depth: usize) {
        self.current_queue_depth.store(depth, Ordering::Release);

        // Update high water mark using compare-and-swap loop
        loop {
            let current_high = self.queue_high_water.load(Ordering::Acquire);
            if depth <= current_high {
                break;
            }
            match self.queue_high_water.compare_exchange_weak(
                current_high,
                depth,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    /// Check if we should scale up workers.
    ///
    /// Returns true if queue depth exceeds threshold and cooldown has elapsed.
    pub fn should_scale_up(&self) -> bool {
        let workers = self.current_worker_count.load(Ordering::Acquire);
        let queue_depth = self.current_queue_depth.load(Ordering::Acquire);

        // Don't exceed max workers
        if workers >= self.config.max_workers {
            return false;
        }

        // Check cooldown
        if !self.cooldown_elapsed() {
            return false;
        }

        // Scale up if queue is deep relative to worker count
        let threshold = (workers as f64 * self.config.scale_up_threshold) as usize;
        queue_depth > threshold
    }

    /// Check if we should scale down workers.
    ///
    /// Returns true if queue is shallow and cooldown has elapsed.
    pub fn should_scale_down(&self) -> bool {
        let workers = self.current_worker_count.load(Ordering::Acquire);
        let queue_depth = self.current_queue_depth.load(Ordering::Acquire);

        // Don't go below min workers
        if workers <= self.config.min_workers {
            return false;
        }

        // Check cooldown
        if !self.cooldown_elapsed() {
            return false;
        }

        // Scale down if queue is shallow relative to worker count
        let threshold = (workers as f64 * self.config.scale_down_threshold) as usize;
        queue_depth < threshold
    }

    /// Get the average items processed per second.
    pub fn throughput(&self) -> f64 {
        let items = self.items_processed.load(Ordering::Acquire);
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            items as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Get the average time per item.
    pub fn avg_item_time(&self) -> Option<Duration> {
        let items = self.items_processed.load(Ordering::Acquire);
        let time_ns = self.total_processing_time_ns.load(Ordering::Acquire);
        if items == 0 {
            return None;
        }
        // Safe division - items is guaranteed non-zero
        Some(Duration::from_nanos(time_ns / items as u64))
    }

    /// Get the current worker count.
    pub fn current_workers(&self) -> usize {
        self.current_worker_count.load(Ordering::Acquire)
    }

    /// Get the current queue depth.
    pub fn queue_depth(&self) -> usize {
        self.current_queue_depth.load(Ordering::Acquire)
    }

    /// Increment the worker count (call after spawning a new worker).
    ///
    /// Returns the new worker count.
    pub fn increment_workers(&self) -> usize {
        let new_count = self.current_worker_count.fetch_add(1, Ordering::AcqRel) + 1;
        self.update_last_adjustment();
        new_count
    }

    /// Decrement the worker count (call after a worker shuts down).
    ///
    /// Returns the new worker count.
    pub fn decrement_workers(&self) -> usize {
        let old = self.current_worker_count.fetch_sub(1, Ordering::AcqRel);
        self.update_last_adjustment();
        old.saturating_sub(1)
    }

    /// Get the suggested worker count adjustment.
    ///
    /// Returns: -1 for scale down, 0 for no change, +1 for scale up.
    pub fn suggested_adjustment(&self) -> i32 {
        if self.should_scale_up() {
            1
        } else if self.should_scale_down() {
            -1
        } else {
            0
        }
    }

    /// Get current metrics as a snapshot.
    pub fn metrics(&self) -> AdaptiveMetrics {
        AdaptiveMetrics {
            current_workers: self.current_workers(),
            items_processed: self.items_processed.load(Ordering::Acquire),
            queue_depth: self.queue_depth(),
            queue_high_water: self.queue_high_water.load(Ordering::Acquire),
            throughput: self.throughput(),
            avg_item_time: self.avg_item_time(),
            elapsed: self.start_time.elapsed(),
        }
    }

    /// Reset all metrics (useful for benchmarking).
    pub fn reset_metrics(&self) {
        self.items_processed.store(0, Ordering::Release);
        self.total_processing_time_ns.store(0, Ordering::Release);
        self.queue_high_water.store(0, Ordering::Release);
        self.current_queue_depth.store(0, Ordering::Release);
    }

    /// Check if the cooldown period has elapsed since last adjustment.
    fn cooldown_elapsed(&self) -> bool {
        let last = self.last_adjustment_nanos.load(Ordering::Acquire);
        let now = self.start_time.elapsed().as_nanos() as u64;
        let cooldown_ns = self.config.adjustment_cooldown.as_nanos() as u64;
        now.saturating_sub(last) >= cooldown_ns
    }

    /// Update the last adjustment timestamp.
    fn update_last_adjustment(&self) {
        let now = self.start_time.elapsed().as_nanos() as u64;
        self.last_adjustment_nanos.store(now, Ordering::Release);
    }

    /// Get the scaling check interval from configuration.
    ///
    /// This determines how often scaling decisions should be made
    /// (e.g., every N items processed).
    pub fn scaling_check_interval(&self) -> usize {
        self.config.scaling_check_interval
    }
}

/// Snapshot of adaptive controller metrics.
#[derive(Debug, Clone)]
pub struct AdaptiveMetrics {
    /// Current number of active workers
    pub current_workers: usize,
    /// Total items processed since start
    pub items_processed: usize,
    /// Current queue depth
    pub queue_depth: usize,
    /// Maximum queue depth observed
    pub queue_high_water: usize,
    /// Current throughput in items per second
    pub throughput: f64,
    /// Average processing time per item
    pub avg_item_time: Option<Duration>,
    /// Total elapsed time since controller creation
    pub elapsed: Duration,
}

impl AdaptiveMetrics {
    /// Check if the pipeline is healthy (workers keeping up with queue).
    pub fn is_healthy(&self) -> bool {
        self.queue_depth <= self.current_workers * 2
    }

    /// Get utilization as a percentage (0.0 to 1.0+).
    ///
    /// Values > 1.0 indicate queue buildup.
    pub fn utilization(&self) -> f64 {
        if self.current_workers == 0 {
            return 0.0;
        }
        self.queue_depth as f64 / self.current_workers as f64
    }
}

/// Manages worker threads with adaptive scaling support.
///
/// Coordinates with the `AdaptiveController` to spawn or signal
/// shutdown of workers based on workload.
pub struct WorkerManager {
    controller: Arc<AdaptiveController>,
    /// Counter for pending shutdown requests
    shutdown_requested: AtomicUsize,
}

impl WorkerManager {
    /// Create a new worker manager with the given adaptive controller.
    pub fn new(controller: Arc<AdaptiveController>) -> Self {
        Self {
            controller,
            shutdown_requested: AtomicUsize::new(0),
        }
    }

    /// Get a reference to the adaptive controller.
    pub fn controller(&self) -> &Arc<AdaptiveController> {
        &self.controller
    }

    /// Check and apply worker adjustments based on controller recommendations.
    ///
    /// Returns the adjustment made: positive for scale-up needed,
    /// negative for scale-down requested, 0 for no change.
    pub fn check_and_adjust(&self) -> i32 {
        let adjustment = self.controller.suggested_adjustment();

        if adjustment > 0 {
            log::debug!(
                "[ADAPTIVE] Recommending scale-up: workers={}, queue={}",
                self.controller.current_workers(),
                self.controller.queue_depth()
            );
        } else if adjustment < 0 {
            // Signal a worker to shut down cooperatively
            self.shutdown_requested.fetch_add(1, Ordering::Release);
            log::debug!(
                "[ADAPTIVE] Requesting scale-down: workers={}, queue={}",
                self.controller.current_workers(),
                self.controller.queue_depth()
            );
        }

        adjustment
    }

    /// Check if a worker should shut down (cooperative shutdown).
    ///
    /// Workers should call this periodically. If it returns true,
    /// the worker should exit its loop and shut down.
    pub fn should_worker_shutdown(&self) -> bool {
        // Try to claim a shutdown request
        loop {
            let requests = self.shutdown_requested.load(Ordering::Acquire);
            if requests == 0 {
                return false;
            }
            match self.shutdown_requested.compare_exchange_weak(
                requests,
                requests - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.controller.decrement_workers();
                    return true;
                }
                Err(_) => continue,
            }
        }
    }

    /// Record that a worker was spawned.
    pub fn worker_spawned(&self) {
        self.controller.increment_workers();
    }

    /// Get current metrics snapshot.
    pub fn metrics(&self) -> AdaptiveMetrics {
        self.controller.metrics()
    }

    /// Get pending shutdown request count.
    pub fn pending_shutdowns(&self) -> usize {
        self.shutdown_requested.load(Ordering::Acquire)
    }
}

// ============================================================================
// Adaptive Scaling Facade
// ============================================================================

/// Unified facade for adaptive scaling functionality.
///
/// This encapsulates `AdaptiveController` (metrics collection) and optionally
/// `WorkerManager` (dynamic scaling) behind a single interface. This reduces
/// feature flag scatter across the codebase and provides a clean API for
/// accessing adaptive functionality.
///
/// # Example
///
/// ```ignore
/// use petty::pipeline::{AdaptiveScalingFacade, AdaptiveConfig};
///
/// // Create facade with default config
/// let facade = AdaptiveScalingFacade::new(4, AdaptiveConfig::default());
///
/// // Access metrics (always available)
/// let metrics = facade.metrics();
/// println!("Items processed: {}", metrics.items_processed);
///
/// // Check if dynamic scaling is available
/// if facade.supports_dynamic_scaling() {
///     // Access worker manager (requires adaptive-scaling feature)
///     #[cfg(feature = "adaptive-scaling")]
///     let manager = facade.manager();
/// }
/// ```
pub struct AdaptiveScalingFacade {
    /// The adaptive controller for metrics collection
    controller: Arc<AdaptiveController>,
    /// Configuration for adaptive behavior
    config: AdaptiveConfig,
    /// Worker manager for dynamic scaling
    manager: Arc<WorkerManager>,
}

impl AdaptiveScalingFacade {
    /// Create a new adaptive scaling facade.
    ///
    /// # Arguments
    ///
    /// * `initial_workers` - Initial number of workers
    /// * `config` - Configuration for adaptive behavior
    pub fn new(initial_workers: usize, config: AdaptiveConfig) -> Self {
        let controller = Arc::new(AdaptiveController::with_config(
            initial_workers,
            config.clone(),
        ));
        let manager = Arc::new(WorkerManager::new(Arc::clone(&controller)));

        Self {
            controller,
            config,
            manager,
        }
    }

    /// Get a reference to the adaptive controller.
    ///
    /// The controller is always available and provides metrics collection.
    pub fn controller(&self) -> &Arc<AdaptiveController> {
        &self.controller
    }

    /// Get a reference to the configuration.
    #[allow(dead_code)] // Public API - tested but not used internally
    pub fn config(&self) -> &AdaptiveConfig {
        &self.config
    }

    /// Get a reference to the worker manager.
    pub fn manager(&self) -> &Arc<WorkerManager> {
        &self.manager
    }

    /// Get the current metrics snapshot.
    pub fn metrics(&self) -> AdaptiveMetrics {
        self.controller.metrics()
    }

    /// Check if dynamic scaling is supported at runtime.
    ///
    /// Returns `true` when the `adaptive-scaling` feature is enabled.
    #[allow(dead_code)] // Public API - tested but not used internally
    pub fn supports_dynamic_scaling(&self) -> bool {
        cfg!(feature = "adaptive-scaling")
    }

    /// Get the scaling check interval from config.
    #[allow(dead_code)] // Public API - tested but not used internally
    pub fn scaling_check_interval(&self) -> usize {
        self.config.scaling_check_interval
    }

    /// Get the max in-flight buffer from config.
    ///
    /// This determines extra headroom for the semaphore permits:
    /// `max_in_flight = workers + max_in_flight_buffer`
    pub fn max_in_flight_buffer(&self) -> usize {
        self.config.max_in_flight_buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_controller_creation() {
        let controller = AdaptiveController::new(4);
        assert_eq!(controller.current_workers(), 4);
        assert_eq!(controller.items_processed.load(Ordering::Acquire), 0);
    }

    #[cfg(feature = "adaptive-scaling")]
    #[test]
    fn test_controller_respects_bounds() {
        let config = AdaptiveConfig::with_worker_bounds(2, 8);
        let controller = AdaptiveController::with_config(1, config);
        // Should be clamped to min
        assert_eq!(controller.current_workers(), 2);

        let config = AdaptiveConfig::with_worker_bounds(2, 8);
        let controller = AdaptiveController::with_config(100, config);
        // Should be clamped to max
        assert_eq!(controller.current_workers(), 8);
    }

    #[test]
    fn test_record_item_processed() {
        let controller = AdaptiveController::new(4);
        controller.record_item_processed(Duration::from_millis(100));
        controller.record_item_processed(Duration::from_millis(200));

        assert_eq!(controller.items_processed.load(Ordering::Acquire), 2);
        let avg_time = controller.avg_item_time().unwrap();
        assert_eq!(avg_time, Duration::from_millis(150));
    }

    #[test]
    fn test_queue_depth_tracking() {
        let controller = AdaptiveController::new(4);
        controller.record_queue_depth(5);
        controller.record_queue_depth(10);
        controller.record_queue_depth(3);

        assert_eq!(controller.queue_depth(), 3);
        assert_eq!(controller.queue_high_water.load(Ordering::Acquire), 10);
    }

    #[cfg(feature = "adaptive-scaling")]
    #[test]
    fn test_scale_up_threshold() {
        let config = AdaptiveConfig {
            min_workers: 2,
            max_workers: 16,
            scale_up_threshold: 2.0,
            scale_down_threshold: 0.25,
            adjustment_cooldown: Duration::from_millis(0), // No cooldown for tests
            scaling_check_interval: 10,
            max_in_flight_buffer: 2,
        };
        let controller = AdaptiveController::with_config(4, config);

        // Queue depth 8 with 4 workers (threshold = 8) should not trigger
        controller.record_queue_depth(8);
        assert!(!controller.should_scale_up());

        // Queue depth 9 with 4 workers (threshold = 8) should trigger
        controller.record_queue_depth(9);
        assert!(controller.should_scale_up());
    }

    #[cfg(feature = "adaptive-scaling")]
    #[test]
    fn test_scale_down_threshold() {
        let config = AdaptiveConfig {
            min_workers: 2,
            max_workers: 16,
            scale_up_threshold: 2.0,
            scale_down_threshold: 0.25,
            adjustment_cooldown: Duration::from_millis(0),
            scaling_check_interval: 10,
            max_in_flight_buffer: 2,
        };
        let controller = AdaptiveController::with_config(4, config);

        // Queue depth 0 with 4 workers (threshold = 1) should trigger
        controller.record_queue_depth(0);
        assert!(controller.should_scale_down());
    }

    #[test]
    fn test_worker_manager_metrics() {
        use std::sync::Arc;
        let controller = Arc::new(AdaptiveController::new(4));
        let manager = WorkerManager::new(controller.clone());

        // Test metrics() accessor
        let metrics = manager.metrics();
        assert_eq!(metrics.current_workers, 4);
        assert_eq!(metrics.items_processed, 0);
    }

    #[cfg(feature = "adaptive-scaling")]
    #[test]
    fn test_worker_manager() {
        use std::sync::Arc;
        let controller = Arc::new(AdaptiveController::new(4));
        let manager = WorkerManager::new(controller.clone());

        assert_eq!(manager.metrics().current_workers, 4);

        // Simulate spawning a worker
        manager.worker_spawned();
        assert_eq!(manager.metrics().current_workers, 5);
    }

    #[test]
    fn test_worker_manager_pending_shutdowns() {
        use std::sync::Arc;
        let controller = Arc::new(AdaptiveController::new(4));
        let manager = WorkerManager::new(controller);

        // Test pending_shutdowns() accessor - should start at 0
        assert_eq!(manager.pending_shutdowns(), 0);
    }

    #[cfg(feature = "adaptive-scaling")]
    #[test]
    fn test_worker_manager_shutdown() {
        use std::sync::Arc;
        let config = AdaptiveConfig {
            min_workers: 2,
            max_workers: 16,
            scale_up_threshold: 2.0,
            scale_down_threshold: 0.5,
            adjustment_cooldown: Duration::from_millis(0),
            scaling_check_interval: 10,
            max_in_flight_buffer: 2,
        };
        let controller = Arc::new(AdaptiveController::with_config(4, config));
        let manager = WorkerManager::new(controller);

        // Empty queue should trigger scale-down
        manager.controller.record_queue_depth(0);
        let adjustment = manager.check_and_adjust();
        assert_eq!(adjustment, -1);
        assert_eq!(manager.pending_shutdowns(), 1);

        // Worker claims shutdown
        assert!(manager.should_worker_shutdown());
        assert_eq!(manager.pending_shutdowns(), 0);
        assert_eq!(manager.metrics().current_workers, 3);
    }

    #[test]
    fn test_metrics_snapshot() {
        let controller = AdaptiveController::new(4);
        controller.record_item_processed(Duration::from_millis(50));
        controller.record_item_processed(Duration::from_millis(50));
        controller.record_queue_depth(3);

        let metrics = controller.metrics();
        assert_eq!(metrics.current_workers, 4);
        assert_eq!(metrics.items_processed, 2);
        assert_eq!(metrics.queue_depth, 3);
        assert!(metrics.avg_item_time.is_some());
    }

    #[test]
    fn test_metrics_health_check() {
        let metrics = AdaptiveMetrics {
            current_workers: 4,
            items_processed: 100,
            queue_depth: 6,
            queue_high_water: 10,
            throughput: 50.0,
            avg_item_time: Some(Duration::from_millis(20)),
            elapsed: Duration::from_secs(2),
        };

        assert!(metrics.is_healthy()); // 6 <= 4*2 = 8

        let unhealthy = AdaptiveMetrics {
            queue_depth: 20,
            ..metrics.clone()
        };
        assert!(!unhealthy.is_healthy()); // 20 > 8
    }

    #[test]
    fn test_reset_metrics() {
        let controller = AdaptiveController::new(4);
        controller.record_item_processed(Duration::from_millis(50));
        controller.record_queue_depth(10);

        controller.reset_metrics();

        let metrics = controller.metrics();
        assert_eq!(metrics.items_processed, 0);
        assert_eq!(metrics.queue_depth, 0);
        assert_eq!(metrics.queue_high_water, 0);
    }

    #[test]
    fn test_config_default_values() {
        let config = AdaptiveConfig::default();
        assert_eq!(config.min_workers, 2);
        assert!(config.max_workers >= 2);
        assert_eq!(config.max_in_flight_buffer, 2);
        #[cfg(feature = "adaptive-scaling")]
        {
            assert_eq!(config.scaling_check_interval, 10);
            assert_eq!(config.adjustment_cooldown, Duration::from_millis(500));
        }
    }

    #[test]
    fn test_controller_config_accessors() {
        let custom_config = AdaptiveConfig {
            min_workers: 2,
            max_workers: 16,
            scale_up_threshold: 2.5,
            scale_down_threshold: 0.3,
            adjustment_cooldown: Duration::from_millis(1000),
            scaling_check_interval: 25,
            max_in_flight_buffer: 3,
        };
        let controller = AdaptiveController::with_config(4, custom_config);

        // Test scaling_check_interval accessor
        assert_eq!(controller.scaling_check_interval(), 25);
    }

    // ========================================
    // Facade Tests
    // ========================================

    #[test]
    fn test_facade_creation() {
        let facade = AdaptiveScalingFacade::new(4, AdaptiveConfig::default());
        assert_eq!(facade.controller().current_workers(), 4);
        assert_eq!(facade.config().min_workers, 2);
        assert_eq!(facade.max_in_flight_buffer(), 2);
    }

    #[test]
    fn test_facade_metrics() {
        let facade = AdaptiveScalingFacade::new(4, AdaptiveConfig::default());
        facade
            .controller()
            .record_item_processed(Duration::from_millis(50));
        facade.controller().record_queue_depth(5);

        let metrics = facade.metrics();
        assert_eq!(metrics.items_processed, 1);
        assert_eq!(metrics.queue_depth, 5);
        assert_eq!(metrics.current_workers, 4);
    }

    #[test]
    fn test_facade_supports_dynamic_scaling() {
        let facade = AdaptiveScalingFacade::new(4, AdaptiveConfig::default());

        #[cfg(feature = "adaptive-scaling")]
        assert!(facade.supports_dynamic_scaling());

        #[cfg(not(feature = "adaptive-scaling"))]
        assert!(!facade.supports_dynamic_scaling());
    }

    #[test]
    fn test_facade_config_accessors() {
        let custom_config = AdaptiveConfig {
            min_workers: 2,
            max_workers: 16,
            scale_up_threshold: 2.5,
            scale_down_threshold: 0.3,
            adjustment_cooldown: Duration::from_millis(1000),
            scaling_check_interval: 15,
            max_in_flight_buffer: 3,
        };
        let facade = AdaptiveScalingFacade::new(4, custom_config);

        // Test config() accessor
        assert_eq!(facade.config().min_workers, 2);
        assert_eq!(facade.config().max_workers, 16);
        assert_eq!(facade.config().scaling_check_interval, 15);

        // Test convenience accessors
        assert_eq!(facade.scaling_check_interval(), 15);
        assert_eq!(facade.max_in_flight_buffer(), 3);
    }

    #[cfg(feature = "adaptive-scaling")]
    #[test]
    fn test_facade_with_manager() {
        let facade = AdaptiveScalingFacade::new(4, AdaptiveConfig::default());

        // Access manager through facade
        let manager = facade.manager();
        assert_eq!(manager.metrics().current_workers, 4);

        // Spawn worker through manager
        manager.worker_spawned();
        assert_eq!(facade.controller().current_workers(), 5);
    }

    #[cfg(feature = "adaptive-scaling")]
    #[test]
    fn test_facade_scaling_check_interval() {
        let mut config = AdaptiveConfig::default();
        config.scaling_check_interval = 20;

        let facade = AdaptiveScalingFacade::new(4, config);
        assert_eq!(facade.scaling_check_interval(), 20);
    }
}
