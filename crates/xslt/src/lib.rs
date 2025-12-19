//! XSLT 1.0 template processor with XML and JSON data source support.
//!
//! This crate provides a full XSLT 1.0 processor with support for both XML and JSON data sources.

pub mod ast;
pub mod compiler;
pub mod executor;
pub mod parser;
pub mod pattern;
pub mod processor;
pub mod util;
pub mod output;
pub mod idf_builder;
pub mod datasources;
pub mod error;

mod compiler_handlers;
mod executor_handlers;

pub use error::{XsltError, Location};
pub use processor::{XsltParser, XsltTemplate};
pub use datasources::{DataSourceNode, NodeType, QName};
