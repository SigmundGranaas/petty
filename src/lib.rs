// --- Module Structure ---
// `pipeline`: The main public API and orchestrator for document generation.
// `error`:    Defines all custom error types for the crate.
// `core`:     Defines stable, primitive data types for styling and layout.
// `idf`:      Defines the Intermediate Representation (IRNode tree).
// `stylesheet`: Defines data structures for parsing stylesheet files.
// `parser`:   Responsible for parsing templates and data into IRNode trees.
// `layout`:   The tree-based, multi-pass engine that produces positioned elements.
// `render`:   Renders positioned elements to a concrete format (e.g., PDF).
// `xpath`:    A simple data selector for JSON, mimicking XPath.

pub mod core;
mod error;
pub mod parser;
mod pipeline;
mod render;
mod xpath;
pub mod templating;
// --- Public API ---
// By exposing only these top-level items, we provide a clean and focused
// public interface for users of the library.

pub use crate::error::PipelineError;
pub use crate::pipeline::{PdfBackend, PipelineBuilder};