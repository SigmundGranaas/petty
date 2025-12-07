// src/core/layout/perf.rs
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

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
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }
}

impl PerformanceTracker {
    pub fn record(&mut self, key: &str, duration: Duration) {
        let entry = self.stats.entry(key.to_string()).or_default();
        entry.update(duration);
    }

    pub fn log_summary(&self, sequence_id: usize) {
        if self.stats.is_empty() {
            return;
        }

        let mut sorted_keys: Vec<_> = self.stats.keys().collect();
        // Sort by total duration descending
        sorted_keys.sort_by(|a, b| {
            self.stats.get(*b).unwrap().total_duration.cmp(&self.stats.get(*a).unwrap().total_duration)
        });

        let mut output = String::new();
        output.push_str(&format!("\n--- Layout Performance Summary (Seq #{}) ---\n", sequence_id));
        output.push_str(&format!(
            "{:<40} | {:<8} | {:<12} | {:<10} | {:<10} | {:<10}\n",
            "Operation", "Calls", "Total", "Avg", "Min", "Max"
        ));
        output.push_str(&format!("{:-<100}\n", ""));

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
        output.push_str("----------------------------------------------------------------------------------------------------\n");

        log::info!("{}", output);
    }

    pub fn reset(&mut self) {
        self.stats.clear();
    }
}