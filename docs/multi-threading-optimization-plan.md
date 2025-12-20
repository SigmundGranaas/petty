# Multi-Threading Pipeline Optimization Plan

## Overview

This plan implements an intelligently-balancing multi-threaded pipeline for the Petty PDF engine, addressing:
- CPU underutilization (currently capped at 2-6 workers)
- Sequential PDF byte generation bottleneck
- Lack of runtime performance metrics
- No adaptive scaling based on workload

## Current Architecture Summary

```
Producer -> Channel -> Workers (2-6 max) -> Channel -> Consumer (sequential PDF)
                         |
              Layout per data item
                         |
                  One worker per item
```

**Key bottlenecks identified:**
- Worker cap at 6 (`streaming.rs:62`)
- Consumer renders pages sequentially (`concurrency.rs:229-251`)
- No sub-item parallelization within workers
- No runtime adaptation

---

## Implementation Phases

### Phase 1: Benchmarking Infrastructure

**Goal:** Establish baselines and enable regression tracking in CI.

#### 1.1 Create Criterion Benchmark Suite

**New files:**
- `benches/pipeline_throughput.rs` - End-to-end pipeline benchmarks
- `benches/layout_performance.rs` - Layout engine micro-benchmarks
- `benches/pdf_generation.rs` - PDF content stream generation benchmarks

**Modify:** `Cargo.toml`
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "pipeline_throughput"
harness = false
```

**Benchmark scenarios:**
- 1, 10, 100, 1000 records throughput
- Worker count scaling (2, 4, 8, 16 workers)
- Content stream generation isolated

#### 1.2 Enhanced Metrics Collection

**Modify:** `crates/layout/src/perf.rs`

Add throughput tracking to `Profiler` trait:
- `record_throughput(items_per_sec: f64)`
- `get_avg_item_time() -> Option<Duration>`

---

### Phase 2: Dynamic Worker Configuration

**Goal:** Remove artificial caps and enable runtime configuration.

#### 2.1 Remove Worker Cap

**Modify:** `src/pipeline/renderer/streaming.rs:62`

```rust
// Before:
let num_layout_threads = num_cpus::get().saturating_sub(1).clamp(2, 6);

// After:
let num_layout_threads = config.worker_count.unwrap_or_else(|| {
    std::env::var("PETTY_WORKER_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| num_cpus::get().saturating_sub(1).max(2))
});
```

**Also modify:** `src/pipeline/provider/metadata.rs:32` (same pattern)

#### 2.2 Pipeline Configuration

**Modify:** `src/pipeline/builder.rs`

Add configuration options:
```rust
pub struct PipelineBuilder {
    // ... existing fields ...
    worker_count: Option<usize>,
    enable_adaptive_scaling: bool,
}

impl PipelineBuilder {
    pub fn with_worker_count(mut self, count: usize) -> Self { ... }
    pub fn with_adaptive_scaling(mut self, enabled: bool) -> Self { ... }
}
```

---

### Phase 3: Parallel PDF Content Generation

**Goal:** Parallelize content stream generation while maintaining streaming order.

**Feature flag:** `parallel-render`

#### 3.1 Parallel Rendering Function

**Modify:** `crates/render-lopdf/src/helpers.rs`

```rust
#[cfg(feature = "parallel-render")]
pub fn render_pages_parallel(
    pages: Vec<Vec<PositionedElement>>,
    font_map: &HashMap<String, String>,
    page_width: f32,
    page_height: f32,
) -> Vec<Result<Content, RenderError>> {
    use rayon::prelude::*;
    pages.into_par_iter()
        .map(|elements| render_elements_to_content(elements, font_map, page_width, page_height))
        .collect()
}
```

#### 3.2 Consumer Integration

**Modify:** `src/pipeline/concurrency.rs:229-251`

Replace sequential page rendering:
```rust
// Before: sequential loop with render_elements_to_content

// After: render in parallel, write sequentially
#[cfg(feature = "parallel-render")]
let contents: Vec<_> = petty_render_lopdf::render_pages_parallel(
    seq.pages, &font_map, page_width, page_height
);
for content_result in contents {
    let content = content_result.map_render_err()?;
    let content_id = writer.write_content_stream(content)?;
    // ... page dict writing (must stay sequential for streaming)
}
```

#### 3.3 Feature Flag

**Modify:** `Cargo.toml`
```toml
[features]
parallel-render = ["rayon"]
```

**Modify:** `crates/render-lopdf/Cargo.toml`
```toml
[features]
parallel-render = ["rayon"]

[dependencies]
rayon = { version = "1.10", optional = true }
```

---

### Phase 4: Self-Balancing Adaptive Controller

**Goal:** Runtime optimization based on throughput metrics (moderate aggressiveness).

#### 4.1 Adaptive Controller

**New file:** `src/pipeline/adaptive.rs`

```rust
pub struct AdaptiveController {
    current_worker_count: AtomicUsize,
    items_processed: AtomicUsize,
    total_processing_time_ns: AtomicU64,
    queue_high_water: AtomicUsize,

    // Configuration (moderate settings)
    min_workers: usize,           // 2
    max_workers: usize,           // num_cpus * 2
    scale_up_threshold: f64,      // Queue > workers * 2
    adjustment_cooldown: Duration, // 500ms between adjustments
}

impl AdaptiveController {
    pub fn record_item_processed(&self, duration: Duration);
    pub fn record_queue_depth(&self, depth: usize);
    pub fn should_scale_up(&self) -> bool;
    pub fn should_scale_down(&self) -> bool;
    pub fn avg_items_per_second(&self) -> f64; // Throughput optimization
}
```

#### 4.2 Worker Manager

**Modify:** `src/pipeline/concurrency.rs`

Add worker spawning/adjustment:
```rust
pub struct WorkerManager {
    controller: Arc<AdaptiveController>,
    active_workers: Mutex<Vec<task::JoinHandle<()>>>,
    rx: async_channel::Receiver<...>,
    tx: async_channel::Sender<...>,
}

impl WorkerManager {
    pub fn adjust_workers(&self, context: &PipelineContext) {
        if self.controller.should_scale_up() {
            // Spawn additional worker
        }
        // Scale-down via cooperative shutdown signal
    }
}
```

#### 4.3 Monitoring Integration

**Modify:** `src/pipeline/renderer/streaming.rs`

Spawn monitoring task when adaptive scaling enabled:
```rust
if config.enable_adaptive_scaling {
    let controller = Arc::new(AdaptiveController::new(num_layout_threads));
    let manager = WorkerManager::new(controller.clone(), ...);

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            manager.adjust_workers(&context);
        }
    });
}
```

---

### Phase 5: Chunked Sequence Processing

**Goal:** Enable parallel processing of items within a batch for better work-stealing.

#### 5.1 Batch Producer

**Modify:** `src/pipeline/concurrency.rs`

Add batched producer variant:
```rust
pub(crate) async fn producer_task_batched<I>(
    data_iterator: I,
    tx: async_channel::Sender<Result<Vec<(usize, Arc<Value>)>, PipelineError>>,
    semaphore: Arc<Semaphore>,
    batch_size: usize, // Configurable, default based on worker count
)
```

#### 5.2 Batch Worker Processing

Workers process batches with internal parallelization:
```rust
while let Ok(batch_result) = rx_clone.recv_blocking() {
    let batch = batch_result?;

    // Process batch items in parallel using rayon
    let results: Vec<_> = batch.into_par_iter()
        .map(|(idx, item)| (idx, process_single_item(idx, item, ...)))
        .collect();

    for (idx, result) in results {
        tx_clone.send_blocking((idx, result))?;
    }
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Add criterion, parallel-render feature |
| `crates/render-lopdf/Cargo.toml` | Add rayon optional dep |
| `crates/render-lopdf/src/helpers.rs` | Add `render_pages_parallel()` |
| `crates/layout/src/perf.rs` | Add throughput metrics |
| `src/pipeline/concurrency.rs` | Parallel consumer, batch producer, worker manager |
| `src/pipeline/renderer/streaming.rs` | Remove worker cap, add adaptive integration |
| `src/pipeline/provider/metadata.rs` | Remove worker cap |
| `src/pipeline/builder.rs` | Add configuration options |
| `src/pipeline/mod.rs` | Add adaptive module |

## New Files

| File | Purpose |
|------|---------|
| `src/pipeline/adaptive.rs` | Self-balancing controller |
| `benches/pipeline_throughput.rs` | Throughput benchmarks |
| `benches/layout_performance.rs` | Layout benchmarks |
| `benches/pdf_generation.rs` | PDF generation benchmarks |

---

## Implementation Order

1. **Phase 1: Benchmarks** - Establish baselines first
2. **Phase 2: Dynamic config** - Remove caps, add configuration
3. **Phase 3: Parallel PDF** - Enable parallel content generation (feature-flagged)
4. **Phase 4: Adaptive scaling** - Self-balancing worker pool
5. **Phase 5: Batch processing** - Chunked sequences for better parallelism

Each phase can be tested and benchmarked independently before proceeding.

---

## Expected Improvements

- **CPU utilization:** From ~50% to 80-95% on multi-core systems
- **Throughput:** 2-4x improvement for large documents
- **Scalability:** Linear scaling with core count (up to I/O bound)
- **Adaptability:** Automatic tuning for varying workloads

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Memory pressure from more workers | Semaphore-based backpressure already in place |
| Rayon/Tokio thread pool conflict | Use `spawn_blocking` for rayon work |
| Regression in small documents | Feature flags allow fallback to current behavior |
