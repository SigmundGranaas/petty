// FILE: src/pipeline/config.rs

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
    /// Automatically select the best strategy. (Default)
    /// Uses Single-Pass if possible, otherwise falls back to Two-Pass.
    #[default]
    Auto,
    /// Force the use of the single-pass streaming strategy.
    /// Will fail if the template contains forward references (ToC, etc.).
    ForceSinglePass,
    /// Force the use of the two-pass strategy.
    /// Slower, but guarantees correctness for all features. Requires a cloneable iterator.
    ForceTwoPass,
    /// Force the use of the hybrid strategy.
    /// Handles all features for non-cloneable iterators by buffering to a temporary file.
    ForceHybrid,
}