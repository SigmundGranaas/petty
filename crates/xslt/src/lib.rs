//! XSLT 1.0 template processor with XML and JSON data source support.
//!
//! This crate provides a full XSLT 1.0 processor with support for both XML and JSON data sources.

pub mod ast;
pub mod compiler;
pub mod datasources;
pub mod error;
pub mod executor;
pub mod idf_builder;
pub mod output;
pub mod parser;
pub mod pattern;
pub mod processor;
pub mod util;

mod compiler_handlers;
mod executor_handlers;

pub use datasources::{DataSourceNode, NodeType, QName};
pub use error::{Location, XsltError};
pub use processor::{XsltParser, XsltTemplate};
