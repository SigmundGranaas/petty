// Reuse the configuration struct defined in the core layout module to ensure synergy.
pub use crate::layout::LayoutConfig as PipelineCacheConfig;

/// An enum to select the desired PDF rendering backend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PdfBackend {
    /// In-memory PDF generation using `printpdf`.
    PrintPdf,
    /// In-memory PDF generation using `printpdf`, with parallel page rendering.
    PrintPdfParallel,
    #[default]
    /// Streaming PDF generation using `lopdf`.
    Lopdf,
    /// Streaming PDF generation using `lopdf`, with parallel page rendering.
    LopdfParallel,
}

/// An enum to select the high-level document generation algorithm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GenerationMode {
    /// Automatically select the best pipeline based on detected template features. (Default)
    /// - If the template has forward references (ToC, Page X of Y, role templates),
    ///   it uses the `MetadataGeneratingProvider` and `ComposingRenderer`.
    /// - Otherwise, it uses the fast `PassThroughProvider` and `SinglePassStreamingRenderer`.
    #[default]
    Auto,
    /// Force the use of the simple, single-pass streaming pipeline.
    /// This will fail if the template contains features that require an analysis pass.
    ForceStreaming,
}

/// Configuration for how work items are processed by the pipeline.
///
/// This controls both metrics collection and adaptive worker scaling behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProcessingMode {
    /// Standard processing: no metrics, no adaptive scaling. (Default)
    ///
    /// This is the default mode and works well for most workloads.
    /// Minimal overhead with fixed worker count.
    #[default]
    Standard,

    /// Collect metrics about pipeline throughput and queue depth.
    ///
    /// Enables the `AdaptiveController` for metrics collection. After processing,
    /// call `DocumentPipeline::metrics()` to retrieve:
    /// - `throughput`: Items processed per second
    /// - `avg_item_time`: Average processing time per item
    /// - `queue_depth`: Current/high-water queue depth
    ///
    /// This has minimal overhead and is useful for benchmarking.
    /// Worker count remains fixed.
    WithMetrics,

    /// Full adaptive scaling: metrics collection + dynamic worker spawning.
    ///
    /// Enables both metrics collection and runtime worker scaling based on
    /// queue depth and throughput. The pipeline will automatically spawn
    /// additional workers when the queue builds up and signal workers to
    /// shut down when the queue is shallow.
    ///
    /// Requires the `adaptive-scaling` feature for dynamic scaling support.
    /// Without the feature, this behaves the same as `WithMetrics`.
    Adaptive,
}
