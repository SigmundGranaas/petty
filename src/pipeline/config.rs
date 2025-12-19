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