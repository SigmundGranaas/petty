//! JSON template engine with JPath expressions.
//!
//! This crate provides a template engine that processes JSON templates
//! with embedded JPath expressions for data selection and transformation.

pub mod ast;
pub mod compiler;
pub mod executor;
pub mod error;
pub mod processor;
mod style_deser;

pub use error::JsonTemplateError;
pub use ast::{TemplateNode, JsonTemplateFile};
pub use compiler::{JsonInstruction, Compiler, CompiledString, CompiledStyles, CompiledTable};
pub use executor::TemplateExecutor;
pub use processor::{JsonParser, CompiledJsonTemplate};