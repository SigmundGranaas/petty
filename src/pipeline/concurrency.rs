// src/pipeline/concurrency.rs
//!
//! Concurrency primitives for the streaming PDF generation pipeline.
//!
//! This module provides producer, worker, and consumer tasks that form the
//! three-stage pipeline for parallel document processing.
//!
//! # Architecture
//!
//! The pipeline uses a three-stage architecture:
//!
//! ```text
//! Producer -> Channel -> Workers (N) -> Channel -> Consumer
//!    |                      |                         |
//! Data items         Layout per item           Render to PDF
//! ```
//!
//! # Adaptive Scaling
//!
//! When an `AdaptiveController` is present in the context:
//! - Workers record item processing times via `record_item_processed()`
//! - Consumer tracks queue depth via `record_queue_depth()`
//! - Metrics can be queried via `DocumentPipeline::metrics()`

use crate::MapRenderError;
use crate::pipeline::adaptive::{AdaptiveController, WorkerManager};
use crate::pipeline::context::PipelineContext;
use crate::pipeline::worker::{LaidOutSequence, finish_layout_and_resource_loading};
use log::{debug, info, warn};
use lopdf::dictionary;
use petty_core::ApiIndexEntry;
use petty_core::error::PipelineError;
use petty_layout::LayoutEngine;
use petty_render_core::DocumentRenderer;
use petty_render_core::{HyperlinkLocation, Pass1Result, ResolvedAnchor};
use petty_render_lopdf::LopdfRenderer;
use petty_template_core::{DataSourceFormat, ExecutionConfig};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::{Seek, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task;

// ============================================================================
// Type Aliases for Channel Types
// ============================================================================

/// Work item sent from producer to workers (index + data).
pub(crate) type WorkItem = Result<(usize, Arc<Value>), PipelineError>;

/// Result sent from workers to consumer (index + layout result).
pub(crate) type LayoutResult = (usize, Result<LaidOutSequence, PipelineError>);

/// Sender for work items.
pub(crate) type WorkItemSender = async_channel::Sender<WorkItem>;

/// Receiver for work items.
pub(crate) type WorkItemReceiver = async_channel::Receiver<WorkItem>;

/// Sender for layout results.
pub(crate) type LayoutResultSender = async_channel::Sender<LayoutResult>;

/// Receiver for layout results.
pub(crate) type LayoutResultReceiver = async_channel::Receiver<LayoutResult>;

// ============================================================================
// Scaling Behavior Trait
// ============================================================================

/// Trait abstracting scaling behavior for the streaming consumer.
///
/// This allows the consumer implementation to be shared between adaptive
/// and non-adaptive modes while delegating scaling decisions to the trait.
pub(crate) trait ScalingBehavior {
    /// Called after each item is received to record queue depth.
    fn on_item_received(&mut self, queue_depth: usize);

    /// Called periodically to check for scaling opportunities.
    ///
    /// # Arguments
    /// * `result_sender` - Optional sender for spawning new workers
    ///
    /// # Returns
    /// Returns true if work is complete and the sender should be dropped.
    fn check_scaling(&mut self, result_sender: Option<&LayoutResultSender>) -> bool;

    /// Record that an item was processed with the given duration.
    fn record_processed(&mut self, duration: Duration);
}

/// No-op scaling behavior for non-adaptive mode.
///
/// Only records metrics if an AdaptiveController is present.
pub(crate) struct NoScaling {
    controller: Option<Arc<AdaptiveController>>,
}

impl NoScaling {
    pub fn new(controller: Option<Arc<AdaptiveController>>) -> Self {
        Self { controller }
    }
}

impl ScalingBehavior for NoScaling {
    fn on_item_received(&mut self, queue_depth: usize) {
        if let Some(ref c) = self.controller {
            c.record_queue_depth(queue_depth);
        }
    }

    fn check_scaling(&mut self, _result_sender: Option<&LayoutResultSender>) -> bool {
        false // Never signals work complete - channel closes naturally
    }

    fn record_processed(&mut self, duration: Duration) {
        if let Some(ref c) = self.controller {
            c.record_item_processed(duration);
        }
    }
}

/// Adaptive scaling behavior with dynamic worker pool support.
pub(crate) struct AdaptiveScaling<'a> {
    pool: &'a mut DynamicWorkerPool,
    controller: Arc<AdaptiveController>,
    check_counter: usize,
    check_interval: usize,
}

impl<'a> AdaptiveScaling<'a> {
    pub fn new(
        pool: &'a mut DynamicWorkerPool,
        controller: Arc<AdaptiveController>,
        check_interval: usize,
    ) -> Self {
        Self {
            pool,
            controller,
            check_counter: 0,
            check_interval,
        }
    }
}

impl ScalingBehavior for AdaptiveScaling<'_> {
    fn on_item_received(&mut self, queue_depth: usize) {
        self.controller.record_queue_depth(queue_depth);
    }

    fn check_scaling(&mut self, result_sender: Option<&LayoutResultSender>) -> bool {
        self.check_counter = self.check_counter.wrapping_add(1);
        if self.check_counter % self.check_interval != 0 {
            return false;
        }

        // Check if work is complete first
        let work_complete = self.pool.is_work_complete();
        if work_complete {
            debug!("[ADAPTIVE] Work complete detected");
            return true; // Signal to drop sender
        }

        // If sender is available, check for scaling opportunities
        if let Some(sender) = result_sender {
            self.pool.check_and_scale(sender);
        }

        false
    }

    fn record_processed(&mut self, duration: Duration) {
        self.controller.record_item_processed(duration);
    }
}

// ============================================================================
// RAII Channel Guards
// ============================================================================

/// RAII guard ensuring channel sender is dropped when guard goes out of scope.
///
/// This replaces manual `drop(sender)` calls with automatic cleanup,
/// ensuring channels close properly even in error paths.
///
/// # Example
///
/// ```ignore
/// let (tx, rx) = async_channel::bounded::<i32>(10);
/// let guard = SenderGuard::new(tx);
///
/// // Clone sender for workers
/// let worker_tx = guard.sender().clone();
///
/// // When guard goes out of scope, channel is closed automatically
/// ```
pub(crate) struct SenderGuard<T> {
    sender: Option<async_channel::Sender<T>>,
}

impl<T> SenderGuard<T> {
    /// Create a new guard taking ownership of the sender.
    pub fn new(sender: async_channel::Sender<T>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    /// Get a reference to the sender for cloning.
    ///
    /// # Panics
    ///
    /// Panics if called after `close()` was called.
    pub fn sender(&self) -> &async_channel::Sender<T> {
        self.sender.as_ref().expect("sender already closed")
    }

    /// Explicitly close the sender before natural drop.
    ///
    /// This is useful when you need to close the channel at a specific point
    /// in the code before the guard goes out of scope.
    #[allow(dead_code)]
    pub fn close(mut self) {
        drop(self.sender.take());
    }
}

impl<T> Drop for SenderGuard<T> {
    fn drop(&mut self) {
        // Sender is dropped automatically when Option is dropped
    }
}

// ============================================================================
// Dynamic Worker Pool (requires adaptive-scaling feature)
// ============================================================================

/// A pool of workers that can dynamically scale based on workload.
///
/// This struct holds all the resources needed to spawn additional workers
/// at runtime when the `adaptive-scaling` feature is enabled.
///
/// # Sender Ownership
///
/// The pool does NOT own the result sender. Instead, the sender should be
/// owned by a `SenderGuard` at the call site. This enables proper channel
/// cleanup via RAII - when the guard goes out of scope after the consumer
/// loop, the channel closes automatically, allowing `recv_blocking()` to
/// return an error and exit cleanly.
pub struct DynamicWorkerPool {
    /// The pipeline context with shared resources
    context: Arc<PipelineContext>,
    /// Receiver for work items (cloned for each new worker)
    work_receiver: WorkItemReceiver,
    /// Worker manager for scaling decisions
    worker_manager: Arc<WorkerManager>,
    /// Handles for dynamically spawned workers
    worker_handles: Vec<task::JoinHandle<()>>,
    /// Counter for worker IDs (starts after initial workers)
    next_worker_id: usize,
}

impl DynamicWorkerPool {
    /// Create a new dynamic worker pool.
    ///
    /// # Arguments
    ///
    /// * `context` - Pipeline context with shared resources
    /// * `work_receiver` - Receiver for work items (will be cloned for new workers)
    /// * `worker_manager` - Manager for scaling decisions
    /// * `initial_worker_count` - Number of initially spawned workers (for ID assignment)
    ///
    /// Note: The result sender is NOT passed here. Instead, pass it to
    /// `check_and_scale()` when called. This allows the sender to be owned
    /// by a `SenderGuard` for proper RAII cleanup.
    pub fn new(
        context: Arc<PipelineContext>,
        work_receiver: WorkItemReceiver,
        worker_manager: Arc<WorkerManager>,
        initial_worker_count: usize,
    ) -> Self {
        Self {
            context,
            work_receiver,
            worker_manager,
            worker_handles: Vec::new(),
            next_worker_id: initial_worker_count,
        }
    }

    /// Check if all work has been received by workers.
    ///
    /// Returns true when the work channel is closed and empty, meaning
    /// all work items have been distributed to workers.
    pub fn is_work_complete(&self) -> bool {
        self.work_receiver.is_closed() && self.work_receiver.is_empty()
    }

    /// Check if scaling is needed and spawn/signal workers accordingly.
    ///
    /// # Arguments
    ///
    /// * `result_sender` - Reference to the result sender for spawning new workers
    ///
    /// Returns the adjustment made: +N for workers spawned, -N for shutdown signals sent.
    pub fn check_and_scale(&mut self, result_sender: &LayoutResultSender) -> i32 {
        // If the work channel is closed and empty, no need to scale
        if self.is_work_complete() {
            return 0;
        }

        let adjustment = self.worker_manager.check_and_adjust();

        if adjustment > 0 {
            // Scale up: spawn a new worker
            self.spawn_worker(result_sender);
        }
        // Scale down is handled by workers checking should_worker_shutdown()

        adjustment
    }

    /// Spawn a new worker and add it to the pool.
    ///
    /// # Arguments
    ///
    /// * `result_sender` - Reference to the result sender to clone for the new worker
    fn spawn_worker(&mut self, result_sender: &LayoutResultSender) {
        let worker_id = self.next_worker_id;
        self.next_worker_id += 1;

        let rx_clone = self.work_receiver.clone();
        let tx_clone = result_sender.clone();
        let current_font_lib = self.context.font_library.clone();
        let template_clone = Arc::clone(&self.context.compiled_template);
        let resource_provider_clone = Arc::clone(&self.context.resource_provider);
        let cache_config = self.context.cache_config;
        let adaptive_controller = self.context.adaptive_controller();
        let worker_manager = Some(Arc::clone(&self.worker_manager));

        let handle = task::spawn_blocking(move || {
            info!(
                "[WORKER-{}] Dynamically spawned for scale-up.",
                worker_id
            );

            let mut layout_engine = LayoutEngine::new(&current_font_lib, cache_config);

            while let Ok(result) = rx_clone.recv_blocking() {
                let item_start = Instant::now();

                let (index, work_result) = match result {
                    Ok((index, context_arc)) => {
                        let data_source_string = serde_json::to_string(&*context_arc).unwrap();
                        let exec_config = ExecutionConfig {
                            format: DataSourceFormat::Json,
                            strict: false,
                        };
                        let layout_result = template_clone
                            .execute(&data_source_string, exec_config)
                            .and_then(|ir_nodes| {
                                finish_layout_and_resource_loading(
                                    worker_id,
                                    ir_nodes,
                                    context_arc.clone(),
                                    resource_provider_clone.as_ref(),
                                    &mut layout_engine,
                                    &template_clone.stylesheet(),
                                    false,
                                )
                            });

                        (index, layout_result)
                    }
                    Err(e) => (0, Err(e)),
                };

                if let Some(ref controller) = adaptive_controller {
                    controller.record_item_processed(item_start.elapsed());
                }

                if tx_clone.send_blocking((index, work_result)).is_err() {
                    warn!("[WORKER-{}] Consumer channel closed.", worker_id);
                    break;
                }

                // Check for cooperative shutdown signal
                if let Some(ref manager) = worker_manager {
                    if manager.should_worker_shutdown() {
                        info!("[WORKER-{}] Received shutdown signal, scaling down.", worker_id);
                        break;
                    }
                }
            }
            info!("[WORKER-{}] Shutting down.", worker_id);
        });

        self.worker_manager.worker_spawned();
        self.worker_handles.push(handle);

        info!(
            "[ADAPTIVE] Spawned worker {}, total workers now: {}",
            worker_id,
            self.worker_manager.controller().current_workers()
        );
    }

    /// Abort all dynamically spawned workers and return their handles.
    pub fn abort_all(self) -> Vec<task::JoinHandle<()>> {
        for handle in &self.worker_handles {
            handle.abort();
        }
        self.worker_handles
    }
}

// ============================================================================
// Rayon Thread Pool Configuration
// ============================================================================

/// Configure the Rayon global thread pool for parallel rendering.
///
/// This should be called early in the application lifecycle, before
/// any parallel rendering operations are performed.
///
/// # Arguments
///
/// * `num_threads` - Number of threads for the Rayon pool (0 = auto-detect)
///
/// # Example
///
/// ```ignore
/// // Use half the available cores for Rayon to avoid conflicting with Tokio
/// configure_rayon_pool(num_cpus::get() / 2);
/// ```
#[cfg(feature = "parallel-render")]
pub fn configure_rayon_pool(num_threads: usize) {
    let threads = if num_threads == 0 {
        // Default: use half the cores to avoid Tokio conflicts
        num_cpus::get().saturating_div(2).max(1)
    } else {
        num_threads
    };

    if let Err(e) = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
    {
        log::warn!(
            "[RAYON] Failed to configure global thread pool: {}. Using default.",
            e
        );
    } else {
        log::debug!("[RAYON] Configured global thread pool with {} threads", threads);
    }
}

// ============================================================================
// Producer Tasks
// ============================================================================

/// Producer task that sends data items to workers.
///
/// Iterates over the data source and sends each item with its index to the
/// worker channel. Uses semaphore-based backpressure to prevent unbounded
/// queue growth.
pub(crate) async fn producer_task<I>(
    data_iterator: I,
    tx: WorkItemSender,
    semaphore: Arc<Semaphore>,
) where
    I: Iterator<Item = Value> + Send + 'static,
{
    info!("[PRODUCER] Starting sequence production from iterator.");
    for (i, item) in data_iterator.enumerate() {
        if let Ok(permit) = semaphore.acquire().await {
            permit.forget();
        }

        if i % 100 == 0 {
            debug!("[PRODUCER] Sending item #{}...", i);
        }
        if tx.send(Ok((i, Arc::new(item)))).await.is_err() {
            warn!("[PRODUCER] Layout channel closed, stopping producer.");
            break;
        }
    }
    info!("[PRODUCER] Finished sequence production.");
}

/// Spawn layout worker threads.
///
/// Each worker pulls items from the input channel, performs layout, and sends
/// results to the output channel. When an `AdaptiveController` is present in
/// the context, workers record processing times for metrics collection.
///
/// When the `adaptive-scaling` feature is enabled and a `WorkerManager` is present,
/// workers will check for cooperative shutdown signals after each item.
///
/// # Arguments
///
/// * `num_threads` - Number of worker threads to spawn
/// * `context` - Pipeline context with shared resources
/// * `rx` - Receiver for work items
/// * `tx` - Sender for layout results
pub(crate) fn spawn_workers(
    num_threads: usize,
    context: &PipelineContext,
    rx: WorkItemReceiver,
    tx: LayoutResultSender,
) -> Vec<task::JoinHandle<()>> {
    let mut handles = Vec::new();
    let cache_config = context.cache_config;

    for worker_id in 0..num_threads {
        let rx_clone = rx.clone();
        let tx_clone = tx.clone();

        // Use the shared font library from the context.
        // This ensures all workers share the same font database and cache,
        // avoiding repeated I/O and cache misses.
        let current_font_lib = context.font_library.clone();
        let template_clone = Arc::clone(&context.compiled_template);
        let resource_provider_clone = Arc::clone(&context.resource_provider);

        // Clone the adaptive controller for metrics recording
        let adaptive_controller = context.adaptive_controller();

        // Clone the worker manager for cooperative shutdown
        let worker_manager = context.worker_manager();

        let worker_handle = task::spawn_blocking(move || {
            info!(
                "[WORKER-{}] Started with shared font library{}.",
                worker_id,
                if adaptive_controller.is_some() { " and metrics" } else { "" }
            );

            let mut layout_engine = LayoutEngine::new(&current_font_lib, cache_config);

            while let Ok(result) = rx_clone.recv_blocking() {
                let item_start = Instant::now();

                let (index, work_result) = match result {
                    Ok((index, context_arc)) => {
                        let data_source_string = serde_json::to_string(&*context_arc).unwrap();
                        let exec_config = ExecutionConfig {
                            format: DataSourceFormat::Json,
                            strict: false,
                        };
                        let layout_result = template_clone
                            .execute(&data_source_string, exec_config)
                            .and_then(|ir_nodes| {
                                finish_layout_and_resource_loading(
                                    worker_id,
                                    ir_nodes,
                                    context_arc.clone(),
                                    resource_provider_clone.as_ref(),
                                    &mut layout_engine,
                                    &template_clone.stylesheet(),
                                    false,
                                )
                            });

                        if let Ok(seq) = &layout_result {
                            let size = seq.rough_heap_size();
                            if size > 2 * 1024 * 1024 {
                                warn!(
                                    "[WORKER-{}] LARGE ITEM #{}: ~{:.2} MB",
                                    worker_id,
                                    index,
                                    size as f64 / 1_000_000.0
                                );
                            }
                        }

                        (index, layout_result)
                    }
                    Err(e) => (0, Err(e)),
                };

                // Record metrics if adaptive controller is present
                if let Some(ref controller) = adaptive_controller {
                    controller.record_item_processed(item_start.elapsed());
                }

                if tx_clone.send_blocking((index, work_result)).is_err() {
                    warn!("[WORKER-{}] Consumer channel closed.", worker_id);
                    break;
                }

                // Check for cooperative shutdown signal (adaptive scaling)
                if let Some(ref manager) = worker_manager {
                    if manager.should_worker_shutdown() {
                        info!("[WORKER-{}] Received shutdown signal, scaling down.", worker_id);
                        break;
                    }
                }
            }
            info!("[WORKER-{}] Shutting down.", worker_id);
        });
        handles.push(worker_handle);
    }
    drop(rx);
    drop(tx);
    handles
}

/// In-order streaming consumer that renders laid-out sequences to PDF.
///
/// This consumer maintains an ordering buffer to ensure pages are rendered
/// in the correct order even when workers complete items out of order.
///
/// When a `DynamicWorkerPool` is provided, the consumer will periodically
/// check for scaling opportunities and spawn/signal workers accordingly.
///
/// # Arguments
///
/// * `rx2` - Receiver for layout results from workers
/// * `renderer` - PDF renderer to write pages to
/// * `page_width` - Page width in points
/// * `page_height` - Page height in points
/// * `perform_analysis` - Whether to collect ToC/index metadata
/// * `semaphore` - Backpressure semaphore
/// * `adaptive_controller` - Optional controller for queue depth tracking
/// * `worker_pool` - Optional dynamic worker pool for scaling
/// * `result_sender` - Optional result sender (will be dropped when work is complete)
pub(crate) fn run_in_order_streaming_consumer<W: Write + Seek + Send + 'static>(
    rx2: LayoutResultReceiver,
    renderer: &mut LopdfRenderer<W>,
    page_width: f32,
    page_height: f32,
    perform_analysis: bool,
    semaphore: Arc<Semaphore>,
    adaptive_controller: Option<Arc<AdaptiveController>>,
    worker_pool: Option<&mut DynamicWorkerPool>,
    mut result_sender: Option<LayoutResultSender>,
) -> Result<(Vec<lopdf::ObjectId>, Pass1Result), PipelineError> {
    // Create appropriate scaling behavior based on whether pool is available
    match (worker_pool, adaptive_controller) {
        (Some(pool), Some(controller)) => {
            // Full adaptive scaling with pool
            let scaling = AdaptiveScaling::new(pool, controller, 10);
            run_consumer_unified(rx2, renderer, page_width, page_height, perform_analysis, semaphore, scaling, &mut result_sender)
        }
        (_, controller) => {
            // No pool - drop sender and use NoScaling
            drop(result_sender);
            let scaling = NoScaling::new(controller);
            run_consumer_unified(rx2, renderer, page_width, page_height, perform_analysis, semaphore, scaling, &mut None)
        }
    }
}

/// Unified internal implementation of the streaming consumer.
///
/// This single implementation handles both adaptive and non-adaptive modes
/// by delegating scaling decisions to the `ScalingBehavior` trait.
///
/// Uses blocking receive for all modes - no polling!
fn run_consumer_unified<W, S>(
    rx2: LayoutResultReceiver,
    renderer: &mut LopdfRenderer<W>,
    page_width: f32,
    page_height: f32,
    perform_analysis: bool,
    semaphore: Arc<Semaphore>,
    mut scaling: S,
    result_sender: &mut Option<LayoutResultSender>,
) -> Result<(Vec<lopdf::ObjectId>, Pass1Result), PipelineError>
where
    W: Write + Seek + Send + 'static,
    S: ScalingBehavior,
{
    let mut buffer = BTreeMap::new();
    let mut next_sequence_idx = 0;
    let mut all_page_ids = Vec::new();
    let mut pass1_result = Pass1Result::default();
    let mut global_page_offset = 0;

    let font_map: HashMap<String, String> = renderer
        .layout_engine
        .registered_fonts()
        .iter()
        .enumerate()
        .map(|(i, font_info)| (font_info.postscript_name.clone(), format!("F{}", i + 1)))
        .collect();

    let mut last_processed_time = Instant::now();

    // Main receive loop - always uses blocking receive (no polling!)
    loop {
        // Check for scaling opportunities and work completion
        // Pass sender so dynamic workers can be spawned if needed
        let work_complete = scaling.check_scaling(result_sender.as_ref());
        if work_complete && result_sender.is_some() {
            drop(result_sender.take());
            debug!("[CONSUMER] Work complete, dropped result sender");
        }

        // Always use blocking receive - efficient and no CPU waste
        let (index, result) = match rx2.recv_blocking() {
            Ok(item) => item,
            Err(_) => break, // Channel closed
        };

        let wait_time = last_processed_time.elapsed();
        if wait_time.as_millis() > 100 {
            debug!("[CONSUMER] Waited {:?} for sequence #{}", wait_time, index);
        }

        buffer.insert(index, result);

        // Record queue depth via scaling behavior
        scaling.on_item_received(buffer.len());

        if buffer.len() > 20 {
            debug!(
                "[CONSUMER] Buffer growing: {} items waiting. Looking for #{}.",
                buffer.len(),
                next_sequence_idx
            );
        }

        // Process buffered items in order
        while let Some(res) = buffer.remove(&next_sequence_idx) {
            let process_start = Instant::now();
            let seq = res?;

            // Analysis pass: collect metadata
            if perform_analysis {
                pass1_result.toc_entries.extend(seq.toc_entries.clone());
                for (name, anchor) in &seq.defined_anchors {
                    pass1_result.resolved_anchors.insert(
                        name.clone(),
                        ResolvedAnchor {
                            global_page_index: global_page_offset + anchor.local_page_index + 1,
                            y_pos: anchor.y_pos,
                        },
                    );
                }
                for (term, locations) in &seq.index_entries {
                    for loc in locations {
                        pass1_result.index_entries.push(ApiIndexEntry {
                            text: term.clone(),
                            page_number: global_page_offset + loc.local_page_index + 1,
                        });
                    }
                }
                for (local_page_idx, page_elements) in seq.pages.iter().enumerate() {
                    let current_global_page_idx = global_page_offset + local_page_idx + 1;
                    for el in page_elements {
                        use petty_layout::LayoutElement;
                        let href = match &el.element {
                            LayoutElement::Text(t) => t.href.as_ref(),
                            _ => None,
                        };
                        if let Some(href_str) = href {
                            if let Some(target_id) = href_str.strip_prefix('#') {
                                pass1_result.hyperlink_locations.push(HyperlinkLocation {
                                    global_page_index: current_global_page_idx,
                                    rect: [el.x, el.y, el.x + el.width, el.y + el.height],
                                    target_id: target_id.to_string(),
                                });
                            }
                        }
                    }
                }
                pass1_result.total_pages += seq.pages.len();
                global_page_offset += seq.pages.len();
            } else if !seq.toc_entries.is_empty() || !seq.index_entries.is_empty() {
                return Err(PipelineError::Config(
                    "Template uses advanced features (ToC/Index) which require Auto (metadata) mode."
                        .into(),
                ));
            }

            renderer.add_resources(&seq.resources).map_render_err()?;

            // Parallel page rendering (when feature is enabled)
            #[cfg(feature = "parallel-render")]
            {
                let contents: Vec<_> = petty_render_lopdf::render_pages_parallel(
                    seq.pages,
                    &font_map,
                    page_width,
                    page_height,
                )
                .map_render_err()?;

                for content_result in contents {
                    let content = content_result.map_render_err()?;
                    let writer = renderer.writer_mut().unwrap();
                    let content_id = writer
                        .write_content_stream(content)
                        .map_err(|e| PipelineError::Render(e.to_string()))?;

                    let page_dict = dictionary! {
                        "Type" => "Page",
                        "Parent" => writer.pages_id,
                        "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
                        "Contents" => content_id,
                        "Resources" => writer.resources_id,
                    };

                    let page_id = writer.write_object(page_dict.into())?;
                    all_page_ids.push(page_id);
                }
            }

            // Sequential page rendering (default)
            #[cfg(not(feature = "parallel-render"))]
            {
                for page_elements in seq.pages {
                    let content = petty_render_lopdf::render_elements_to_content(
                        page_elements,
                        &font_map,
                        page_width,
                        page_height,
                    )
                    .map_render_err()?;
                    let writer = renderer.writer_mut().unwrap();
                    let content_id = writer
                        .write_content_stream(content)
                        .map_err(|e| PipelineError::Render(e.to_string()))?;

                    let page_dict = dictionary! {
                        "Type" => "Page",
                        "Parent" => writer.pages_id,
                        "MediaBox" => vec![0.0.into(), 0.0.into(), page_width.into(), page_height.into()],
                        "Contents" => content_id,
                        "Resources" => writer.resources_id,
                    };

                    let page_id = writer.write_object(page_dict.into())?;
                    all_page_ids.push(page_id);
                }
            }

            // Record processing time and release semaphore
            scaling.record_processed(process_start.elapsed());
            semaphore.add_permits(1);
            next_sequence_idx += 1;
            last_processed_time = Instant::now();
        }
    }

    Ok((all_page_ids, pass1_result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sender_guard_drops_sender() {
        let (tx, rx) = async_channel::bounded::<i32>(1);
        {
            let _guard = SenderGuard::new(tx);
            // Guard holds sender, channel still open
            assert!(!rx.is_closed());
        }
        // Guard dropped, sender dropped, channel closed
        assert!(rx.is_closed());
    }

    #[test]
    fn test_sender_guard_clone_works() {
        let (tx, rx) = async_channel::bounded::<i32>(1);
        let guard = SenderGuard::new(tx);

        // Clone the sender through the guard
        let cloned = guard.sender().clone();

        // Send through cloned sender
        cloned.send_blocking(42).unwrap();
        assert_eq!(rx.recv_blocking().unwrap(), 42);

        // Original guard still valid
        guard.sender().send_blocking(43).unwrap();
        assert_eq!(rx.recv_blocking().unwrap(), 43);
    }

    #[test]
    fn test_sender_guard_close() {
        let (tx, rx) = async_channel::bounded::<i32>(1);
        let guard = SenderGuard::new(tx);

        assert!(!rx.is_closed());
        guard.close();
        assert!(rx.is_closed());
    }

    #[test]
    fn test_sender_guard_with_workers() {
        // Simulate the worker pattern: guard holds sender, workers get clones
        let (tx, rx) = async_channel::bounded::<i32>(10);
        let guard = SenderGuard::new(tx);

        // "Spawn" workers with cloned senders
        let worker_tx1 = guard.sender().clone();
        let worker_tx2 = guard.sender().clone();

        // Workers can send
        worker_tx1.send_blocking(1).unwrap();
        worker_tx2.send_blocking(2).unwrap();

        // Drop worker senders
        drop(worker_tx1);
        drop(worker_tx2);

        // Channel still open because guard holds original
        assert!(!rx.is_closed());

        // Drop guard - now channel closes
        drop(guard);
        assert!(rx.is_closed());

        // Verify messages were received
        assert_eq!(rx.recv_blocking().unwrap(), 1);
        assert_eq!(rx.recv_blocking().unwrap(), 2);
        assert!(rx.recv_blocking().is_err()); // Channel closed
    }

    // ========================================
    // ScalingBehavior Tests
    // ========================================

    #[test]
    fn test_no_scaling_without_controller() {
        let mut scaling = NoScaling::new(None);

        // All operations should be no-ops without panicking
        scaling.on_item_received(10);
        assert!(!scaling.check_scaling(None));
        scaling.record_processed(Duration::from_millis(50));
    }

    #[test]
    fn test_no_scaling_with_controller() {
        use crate::pipeline::adaptive::AdaptiveController;

        let controller = Arc::new(AdaptiveController::new(4));
        let mut scaling = NoScaling::new(Some(Arc::clone(&controller)));

        // Should record metrics
        scaling.on_item_received(5);
        assert_eq!(controller.queue_depth(), 5);

        scaling.record_processed(Duration::from_millis(100));
        assert_eq!(controller.metrics().items_processed, 1);

        // Should never trigger scaling
        assert!(!scaling.check_scaling(None));
    }

    #[test]
    fn test_no_scaling_never_requests_polling() {
        let mut scaling = NoScaling::new(None);

        // Check multiple times - should never signal work complete
        for _ in 0..100 {
            scaling.on_item_received(50);
            assert!(!scaling.check_scaling(None));
        }
    }
}
