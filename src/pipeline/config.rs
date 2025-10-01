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