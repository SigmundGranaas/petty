// --- Module Structure ---
// `pipeline`: The main public API and orchestrator for document generation.
// `error`:    Defines all custom error types for the crate.
// `idf`:      Defines the Intermediate Representation (IRNode tree).
// `stylesheet`: Defines data structures for parsing stylesheet files.
// `parser`:   Responsible for parsing templates and data into IRNode trees.
// `layout`:   The tree-based, multi-pass engine that produces positioned elements.
// `render`:   Renders positioned elements to a concrete format (e.g., PDF).
// `xpath`:    A simple data selector for JSON, mimicking XPath.

mod error;
mod idf;
mod layout;
mod parser;
mod pipeline;
mod render;
pub mod stylesheet;
mod xpath;

// --- Public API ---
// By exposing only these top-level items, we provide a clean and focused
// public interface for users of the library.

pub use error::PipelineError;
pub use pipeline::PipelineBuilder;