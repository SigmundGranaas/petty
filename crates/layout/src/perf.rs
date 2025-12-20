use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
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
}

/// Real profiler implementation.
/// Only compiled/used when explicitly enabled or for debugging.
pub struct DebugProfiler {
    stats: Mutex<HashMap<String, Duration>>,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl DebugProfiler {
    pub fn new() -> Self {
        Self {
            stats: Mutex::new(HashMap::new()),
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
        }
    }

    pub fn log_summary(&self, sequence_id: usize) {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
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

        if let Ok(stats) = self.stats.lock() {
            for (k, v) in stats.iter() {
                log::info!("{}: {:?}", k, v);
            }
        }
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
        self.hits.fetch_add(1, Ordering::Relaxed);
    }
    fn count_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }
    fn reset(&self) {
        if let Ok(mut g) = self.stats.lock() {
            g.clear();
        }
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }
}
