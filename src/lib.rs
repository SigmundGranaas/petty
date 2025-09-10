// --- Module Structure ---
// `pipeline`: The main public API and orchestrator for document generation.
// `error`:    Defines all custom error types for the crate.
// `stylesheet`: Defines the data structures for parsing stylesheet JSON files.
// `parser`:   Responsible for parsing stylesheet templates and data into an intermediate event stream.
// `layout`:   Processes the event stream to calculate element positions and handle page breaks.
// `render`:   Takes positioned elements from the layout engine and renders them to a concrete format (e.g., PDF).
// `xpath`:    A simple data selector for JSON, mimicking XPath.

mod error;
mod layout;
mod parser;
mod pipeline;
mod render;
pub mod stylesheet;
mod xpath;
mod idf;
// --- Public API ---
// By exposing only these top-level items, we provide a clean and focused
// public interface for users of the library. The internal workings of the
// parser, layout, and render modules are kept private.

pub use error::PipelineError;
pub use pipeline::PipelineBuilder;