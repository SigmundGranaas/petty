use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

/// Trait for measuring layout performance.
///
/// This allows us to swap implementations. In production/release builds
/// without the "profiling" feature, these methods can be optimized to no-ops.
pub trait Profiler: Send + Sync {
    fn record(&self, key: &str, duration: Duration);
    fn count_hit(&self);
    fn count_miss(&self);
    fn reset(&self);

    /// Record throughput measurement (items processed per second)
    fn record_throughput(&self, items_per_sec: f64);

    /// Get the average time per item processed
    fn get_avg_item_time(&self) -> Option<Duration>;
}

/// A no-op profiler for production use.
/// The compiler will inline these and eliminate the overhead.
pub struct NoOpProfiler;

impl Profiler for NoOpProfiler {
    #[inline(always)]
    fn record(&self, _key: &str, _duration: Duration) {}
    #[inline(always)]
    fn count_hit(&self) {}
    #[inline(always)]
    fn count_miss(&self) {}
    #[inline(always)]
    fn reset(&self) {}
    #[inline(always)]
    fn record_throughput(&self, _items_per_sec: f64) {}
    #[inline(always)]
    fn get_avg_item_time(&self) -> Option<Duration> {
        None
    }
}

/// Real profiler implementation.
/// Only compiled/used when explicitly enabled or for debugging.
pub struct DebugProfiler {
    stats: Mutex<HashMap<String, Duration>>,
    hits: AtomicUsize,
    misses: AtomicUsize,
    /// Total items processed for throughput calculation
    items_processed: AtomicUsize,
    /// Total processing time in nanoseconds for throughput calculation
    total_processing_time_ns: AtomicU64,
}

impl DebugProfiler {
    pub fn new() -> Self {
        Self {
            stats: Mutex::new(HashMap::new()),
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
            items_processed: AtomicUsize::new(0),
            total_processing_time_ns: AtomicU64::new(0),
        }
    }

    pub fn log_summary(&self, sequence_id: usize) {
        let hits = self.hits.load(Ordering::Acquire);
        let misses = self.misses.load(Ordering::Acquire);
        let total = hits + misses;
        if total == 0 {
            return;
        }

        log::info!("=== Profile Summary (ID: {}) ===", sequence_id);
        log::info!(
            "Cache Hits: {} ({:.1}%)",
            hits,
            (hits as f64 / total as f64) * 100.0
        );

        // Log throughput metrics
        let items = self.items_processed.load(Ordering::Acquire);
        let time_ns = self.total_processing_time_ns.load(Ordering::Acquire);
        if items > 0 && time_ns > 0 {
            let time_secs = time_ns as f64 / 1_000_000_000.0;
            let throughput = items as f64 / time_secs;
            log::info!(
                "Throughput: {:.2} items/sec ({} items in {:.3}s)",
                throughput,
                items,
                time_secs
            );
        }

        if let Ok(stats) = self.stats.lock() {
            for (k, v) in stats.iter() {
                log::info!("{}: {:?}", k, v);
            }
        }
    }

    /// Record that an item was processed with the given duration.
    ///
    /// Uses Release ordering for cross-thread visibility.
    pub fn record_item_processed(&self, duration: Duration) {
        self.items_processed.fetch_add(1, Ordering::Release);
        // Saturating conversion to prevent overflow on very long durations
        let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        self.total_processing_time_ns.fetch_add(nanos, Ordering::Release);
    }

    /// Get the current throughput in items per second.
    pub fn current_throughput(&self) -> f64 {
        let items = self.items_processed.load(Ordering::Acquire);
        let time_ns = self.total_processing_time_ns.load(Ordering::Acquire);
        if time_ns == 0 {
            return 0.0;
        }
        let time_secs = time_ns as f64 / 1_000_000_000.0;
        items as f64 / time_secs
    }
}

impl Default for DebugProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler for DebugProfiler {
    fn record(&self, key: &str, duration: Duration) {
        if let Ok(mut g) = self.stats.lock() {
            *g.entry(key.to_string()).or_default() += duration;
        }
    }
    fn count_hit(&self) {
        self.hits.fetch_add(1, Ordering::Release);
    }

    fn count_miss(&self) {
        self.misses.fetch_add(1, Ordering::Release);
    }

    fn reset(&self) {
        if let Ok(mut g) = self.stats.lock() {
            g.clear();
        }
        self.hits.store(0, Ordering::Release);
        self.misses.store(0, Ordering::Release);
        self.items_processed.store(0, Ordering::Release);
        self.total_processing_time_ns.store(0, Ordering::Release);
    }

    fn record_throughput(&self, _items_per_sec: f64) {
        // Throughput is calculated from items_processed and total_processing_time_ns.
        // This method exists for API compatibility but actual tracking uses
        // record_item_processed() for more accurate measurements.
    }

    fn get_avg_item_time(&self) -> Option<Duration> {
        let items = self.items_processed.load(Ordering::Acquire);
        let time_ns = self.total_processing_time_ns.load(Ordering::Acquire);
        if items == 0 {
            return None;
        }
        Some(Duration::from_nanos(time_ns / items as u64))
    }
}
