//! Defines the `OutputBuilder` trait, which decouples the XSLT executor
//! from the specific output tree format (e.g., IDF).
//!
//! Also provides the `OutputSink` trait for multi-document output support
//! (e.g., `xsl:result-document` in XSLT 3.0).

use super::ast::PreparsedStyles;
use petty_idf::IRNode;
use petty_style::dimension::Dimension;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

// ============================================================================
// Output Error Types
// ============================================================================

/// Error type for output operations in multi-document transformations.
#[derive(Debug, Clone)]
pub enum OutputError {
    /// Failed to create output for the given href
    CreateFailed {
        /// The href that failed
        href: String,
        /// The reason for failure
        reason: String,
    },
    /// Failed to write to output
    WriteFailed {
        /// The href that failed
        href: String,
        /// The reason for failure
        reason: String,
    },
    /// Duplicate href in same transformation (XTDE1490)
    DuplicateHref {
        /// The duplicated href
        href: String,
    },
    /// Output not allowed in current context
    NotAllowed {
        /// The reason it's not allowed
        reason: String,
    },
    /// Nested result-document with same href (XTDE1500)
    NestedConflict {
        /// The conflicting href
        href: String,
    },
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::CreateFailed { href, reason } => {
                write!(f, "Failed to create output for '{}': {}", href, reason)
            }
            OutputError::WriteFailed { href, reason } => {
                write!(f, "Failed to write to output '{}': {}", href, reason)
            }
            OutputError::DuplicateHref { href } => {
                write!(
                    f,
                    "Duplicate xsl:result-document with href '{}' (XTDE1490)",
                    href
                )
            }
            OutputError::NotAllowed { reason } => {
                write!(f, "Output not allowed: {}", reason)
            }
            OutputError::NestedConflict { href } => {
                write!(
                    f,
                    "Nested xsl:result-document with same href '{}' (XTDE1500)",
                    href
                )
            }
        }
    }
}

impl std::error::Error for OutputError {}

// ============================================================================
// Output Sink Traits
// ============================================================================

/// A sink that can receive multiple output documents.
///
/// This trait enables streaming multi-output transformations where each
/// `xsl:result-document` creates a separate output stream.
///
/// # Streaming Semantics
///
/// Each output document is processed independently and can be flushed
/// immediately when complete. This maintains Petty's streaming architecture
/// with bounded memory usage.
///
/// # Example
///
/// ```ignore
/// let sink = MultiOutputCollector::new();
/// executor.with_output_sink(Arc::new(sink.clone()));
/// executor.build_tree()?;
///
/// // Access collected outputs
/// for (href, nodes) in sink.take_outputs() {
///     // Process each output...
/// }
/// ```
pub trait OutputSink: Send + Sync {
    fn create_output(&self, href: &str) -> Result<Box<dyn DocumentOutput>, OutputError>;

    fn has_href(&self, href: &str) -> bool;
}

/// A single output document being written.
///
/// Represents one result document (either primary or from `xsl:result-document`).
/// Content is streamed to this output via the `OutputBuilder` interface.
pub trait DocumentOutput: Send {
    /// Get the mutable `OutputBuilder` for writing content.
    fn builder(&mut self) -> &mut dyn OutputBuilder;

    /// Complete this document and release resources.
    ///
    /// This is called when the `xsl:result-document` body completes.
    /// Implementations should flush any buffers and finalize the document.
    fn finish(self: Box<Self>) -> Result<(), OutputError>;
}

// ============================================================================
// Multi-Output Collector Implementation
// ============================================================================

/// A collected output from a transformation.
#[derive(Debug, Clone)]
pub struct CollectedOutput {
    /// The href for this output (empty string for primary)
    pub href: String,
    /// The IR nodes for this output
    pub nodes: Vec<IRNode>,
}

/// An `OutputSink` that collects all outputs in memory.
///
/// This is useful for testing and for pipelines that process
/// outputs after the transformation completes.
///
/// # Thread Safety
///
/// This implementation uses interior mutability via `Mutex` to allow
/// multiple outputs to be collected during a transformation.
#[derive(Clone, Default)]
pub struct MultiOutputCollector {
    outputs: Arc<Mutex<HashMap<String, Vec<IRNode>>>>,
    created: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl MultiOutputCollector {
    /// Create a new empty collector.
    pub fn new() -> Self {
        Self {
            outputs: Arc::new(Mutex::new(HashMap::new())),
            created: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Take all collected outputs, leaving the collector empty.
    ///
    /// Returns a vector of `CollectedOutput` containing href and nodes.
    pub fn take_outputs(&self) -> Vec<CollectedOutput> {
        let mut outputs = self.outputs.lock().unwrap();
        outputs
            .drain()
            .map(|(href, nodes)| CollectedOutput { href, nodes })
            .collect()
    }

    /// Get a reference to outputs without consuming them.
    pub fn get_outputs(&self) -> HashMap<String, Vec<IRNode>> {
        self.outputs.lock().unwrap().clone()
    }

    /// Store the result for an href.
    fn store_output(&self, href: String, nodes: Vec<IRNode>) {
        self.outputs.lock().unwrap().insert(href, nodes);
    }
}

impl OutputSink for MultiOutputCollector {
    fn create_output(&self, href: &str) -> Result<Box<dyn DocumentOutput>, OutputError> {
        let mut created = self.created.lock().unwrap();

        if created.contains(href) {
            return Err(OutputError::DuplicateHref {
                href: href.to_string(),
            });
        }

        created.insert(href.to_string());

        Ok(Box::new(CollectorDocumentOutput {
            href: href.to_string(),
            builder: crate::idf_builder::IdfBuilder::new(),
            collector: self.clone(),
        }))
    }

    fn has_href(&self, href: &str) -> bool {
        self.created.lock().unwrap().contains(href)
    }
}

impl fmt::Debug for MultiOutputCollector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let outputs = self.outputs.lock().unwrap();
        let created = self.created.lock().unwrap();
        f.debug_struct("MultiOutputCollector")
            .field("output_count", &outputs.len())
            .field("created_hrefs", &created.len())
            .finish()
    }
}

/// A `DocumentOutput` that collects to a `MultiOutputCollector`.
struct CollectorDocumentOutput {
    href: String,
    builder: crate::idf_builder::IdfBuilder,
    collector: MultiOutputCollector,
}

impl DocumentOutput for CollectorDocumentOutput {
    fn builder(&mut self) -> &mut dyn OutputBuilder {
        &mut self.builder
    }

    fn finish(self: Box<Self>) -> Result<(), OutputError> {
        let nodes = self.builder.get_result();
        self.collector.store_output(self.href, nodes);
        Ok(())
    }
}

// ============================================================================
// Single Output Sink (Default/Backward Compatible)
// ============================================================================

/// A simple sink that only supports the primary output.
///
/// This is the default behavior for transformations that don't use
/// `xsl:result-document` with non-empty href. Any attempt to write
/// to a secondary output will produce an error.
#[derive(Debug, Default)]
pub struct SingleOutputSink {
    primary_created: Mutex<bool>,
}

impl SingleOutputSink {
    /// Create a new single output sink.
    pub fn new() -> Self {
        Self {
            primary_created: Mutex::new(false),
        }
    }
}

impl OutputSink for SingleOutputSink {
    fn create_output(&self, href: &str) -> Result<Box<dyn DocumentOutput>, OutputError> {
        // Only allow empty href or "#default"
        let is_primary = href.is_empty() || href == "#default";

        if !is_primary {
            return Err(OutputError::NotAllowed {
                reason: format!(
                    "xsl:result-document with href='{}' requires an OutputSink that supports \
                     multi-output. Use MultiOutputCollector or provide a custom implementation.",
                    href
                ),
            });
        }

        let mut created = self.primary_created.lock().unwrap();
        if *created {
            return Err(OutputError::DuplicateHref {
                href: href.to_string(),
            });
        }
        *created = true;

        Ok(Box::new(CollectorDocumentOutput {
            href: href.to_string(),
            builder: crate::idf_builder::IdfBuilder::new(),
            collector: MultiOutputCollector::new(),
        }))
    }

    fn has_href(&self, href: &str) -> bool {
        if href.is_empty() || href == "#default" {
            *self.primary_created.lock().unwrap()
        } else {
            false
        }
    }
}

/// A trait that describes the semantic actions of building the output tree,
/// without exposing the underlying concrete node types.
pub trait OutputBuilder {
    // --- Block-level elements ---
    fn start_block(&mut self, styles: &PreparsedStyles);
    fn end_block(&mut self);

    fn start_flex_container(&mut self, styles: &PreparsedStyles);
    fn end_flex_container(&mut self);

    fn start_paragraph(&mut self, styles: &PreparsedStyles);
    fn end_paragraph(&mut self);

    fn start_list(&mut self, styles: &PreparsedStyles);
    fn end_list(&mut self);

    fn start_list_item(&mut self, styles: &PreparsedStyles);
    fn end_list_item(&mut self);

    fn start_image(&mut self, styles: &PreparsedStyles);
    fn end_image(&mut self);

    // --- Table elements ---
    fn start_table(&mut self, styles: &PreparsedStyles);
    fn end_table(&mut self);
    fn start_table_header(&mut self);
    fn end_table_header(&mut self);
    fn set_table_columns(&mut self, columns: &[Dimension]);
    fn start_table_row(&mut self, styles: &PreparsedStyles);
    fn end_table_row(&mut self);
    fn start_table_cell(&mut self, styles: &PreparsedStyles);
    fn end_table_cell(&mut self);

    // --- Inline-level elements ---
    fn add_text(&mut self, text: &str);

    // --- Special elements ---
    fn start_heading(&mut self, styles: &PreparsedStyles, level: u8);
    fn end_heading(&mut self);
    fn add_page_break(&mut self, master_name: Option<String>);

    fn start_styled_span(&mut self, styles: &PreparsedStyles);
    fn end_styled_span(&mut self);

    fn start_hyperlink(&mut self, styles: &PreparsedStyles);
    fn end_hyperlink(&mut self);

    // --- Attributes ---
    /// Sets an attribute on the currently open element.
    fn set_attribute(&mut self, name: &str, value: &str);
}
