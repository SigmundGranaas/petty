// FILE: src/parser/xslt/mod.rs
// src/parser/xslt/mod.rs
pub mod ast;
pub mod compiler;
pub mod executor;
pub(crate) mod util;

use crate::parser::ParseError;
use ast::{PreparsedTemplate, XsltInstruction};

/// Finds the `<page-sequence>` tag within a compiled root template and
/// returns its body. This is used by the "pull" model to define the template
/// for each item in a sequence.
///
/// **DEPRECATED:** This function is part of the old "pull" model. The new "push" model
/// starts execution from the root template (`match="/"`) directly. This function is
/// kept for backward compatibility during the transition but will be removed.
pub fn find_and_extract_sequence_body(
    root_template: &PreparsedTemplate,
) -> Result<PreparsedTemplate, ParseError> {
    for instruction in &root_template.0 {
        if let XsltInstruction::ContentTag {
            tag_name, body, ..
        } = instruction
        {
            if tag_name == b"page-sequence" {
                return Ok(body.clone());
            }
        }
    }
    // In the new model, page-sequence is no longer strictly required at the top level.
    // If it's missing, we can assume the entire root template is the sequence body.
    Ok(root_template.clone())
}