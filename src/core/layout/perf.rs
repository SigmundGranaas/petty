use std::collections::HashMap;
use std::time::Duration;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default, Clone)]
struct StatEntry {
    count: u64,
    total_duration: Duration,
    min_duration: Duration,
    max_duration: Duration,
}

impl StatEntry {
    fn update(&mut self, duration: Duration) {
        if self.count == 0 {
            self.min_duration = duration;
            self.max_duration = duration;
        } else {
            if duration < self.min_duration {
                self.min_duration = duration;
            }
            if duration > self.max_duration {
                self.max_duration = duration;
            }
        }
        self.count += 1;
        self.total_duration += duration;
    }
}

pub struct PerformanceTracker {
    stats: HashMap<String, StatEntry>,
    pub cache_hits: AtomicUsize,
    pub cache_misses: AtomicUsize,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self {
            stats: HashMap::new(),
            cache_hits: AtomicUsize::new(0),
            cache_misses: AtomicUsize::new(0),
        }
    }
}

impl PerformanceTracker {
    pub fn record(&mut self, key: &str, duration: Duration) {
        let entry = self.stats.entry(key.to_string()).or_default();
        entry.update(duration);
    }

    pub fn count_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn count_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn log_summary(&self, sequence_id: usize) {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total_reqs = hits + misses;

        // Optimization: Don't log empty reports (e.g. from unused engines or main thread)
        if total_reqs == 0 && self.stats.is_empty() {
            return;
        }

        let hit_ratio = if total_reqs > 0 {
            (hits as f64 / total_reqs as f64) * 100.0
        } else {
            0.0
        };
        let miss_ratio = if total_reqs > 0 { 100.0 - hit_ratio } else { 0.0 };

        let mut output = String::new();
        output.push_str(&format!("\n=== Layout Performance Report (ID: {}) ===\n", sequence_id));

        // Cache Statistics (Thrashing Rate)
        output.push_str("Cache Performance:\n");
        output.push_str(&format!("  Requests: {}\n", total_reqs));
        output.push_str(&format!("  Hits:     {} ({:.2}%)\n", hits, hit_ratio));
        output.push_str(&format!("  Misses:   {} ({:.2}%) [Thrashing Rate]\n", misses, miss_ratio));

        // High-level Averages
        output.push_str("Key Metrics:\n");

        if let Some(seq_stats) = self.stats.get("LayoutEngine::build_render_tree") {
            let avg_seq = if seq_stats.count > 0 {
                seq_stats.total_duration / seq_stats.count as u32
            } else {
                Duration::ZERO
            };
            output.push_str(&format!("  Avg Time Per Sequence: {:?}\n", avg_seq));
            output.push_str(&format!("  Total Sequences:       {}\n", seq_stats.count));
        }

        if let Some(page_stats) = self.stats.get("PageLayout::generate_page") {
            let avg_page = if page_stats.count > 0 {
                page_stats.total_duration / page_stats.count as u32
            } else {
                Duration::ZERO
            };
            output.push_str(&format!("  Avg Time Per Page:     {:?}\n", avg_page));
            output.push_str(&format!("  Total Pages:           {}\n", page_stats.count));
        }

        // Detailed Breakdown
        if !self.stats.is_empty() {
            output.push_str("\nDetailed Operation Breakdown:\n");
            let mut sorted_keys: Vec<_> = self.stats.keys().collect();
            // Sort by total duration descending
            sorted_keys.sort_by(|a, b| {
                self.stats.get(*b).unwrap().total_duration.cmp(&self.stats.get(*a).unwrap().total_duration)
            });

            output.push_str(&format!(
                "{:<40} | {:<8} | {:<12} | {:<10} | {:<10} | {:<10}\n",
                "Operation", "Calls", "Total", "Avg", "Min", "Max"
            ));
            output.push_str(&format!("{:-<105}\n", ""));

            for key in sorted_keys {
                let entry = self.stats.get(key).unwrap();
                let avg = if entry.count > 0 {
                    entry.total_duration / entry.count as u32
                } else {
                    Duration::ZERO
                };

                output.push_str(&format!(
                    "{:<40} | {:<8} | {:<12?} | {:<10?} | {:<10?} | {:<10?}\n",
                    key, entry.count, entry.total_duration, avg, entry.min_duration, entry.max_duration
                ));
            }
        }
        output.push_str("=========================================================\n");

        log::info!("{}", output);
    }

    pub fn reset(&mut self) {
        self.stats.clear();
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
    }
}