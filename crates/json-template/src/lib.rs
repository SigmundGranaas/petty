//! JSON template engine with JPath expressions.
//!
//! This crate provides a template engine that processes JSON templates
//! with embedded JPath expressions for data selection and transformation.

pub mod ast;
pub mod compiler;
pub mod error;
pub mod executor;
pub mod processor;
mod style_deser;

pub use ast::{JsonTemplateFile, TemplateNode};
pub use compiler::{CompiledString, CompiledStyles, CompiledTable, Compiler, JsonInstruction};
pub use error::JsonTemplateError;
pub use executor::TemplateExecutor;
pub use processor::{CompiledJsonTemplate, JsonParser};
