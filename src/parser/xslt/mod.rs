// FILE: src/parser/xslt/mod.rs
pub mod ast;
pub mod compiler;
pub mod executor;
pub mod util;

use crate::core::idf::IRNode;
use crate::parser::ParseError;
use handlebars::Handlebars;
use serde_json::Value;

/// The main public entry point for the XSLT parser module.
/// It orchestrates the compile and execute phases for an XSLT template.
pub fn process_xslt<'h>(
    xslt_content: &str,
    data: &Value,
    handlebars: &'h Handlebars<'h>,
) -> Result<Vec<IRNode>, ParseError> {
    // Phase 1: Compile the XSLT source into an executable structure.
    let compiled_stylesheet = compiler::Compiler::compile(xslt_content)?;

    // Phase 2: Execute the compiled stylesheet against the data context.
    let mut executor = executor::TemplateExecutor::new(handlebars, &compiled_stylesheet);
    executor.build_tree(data)
}