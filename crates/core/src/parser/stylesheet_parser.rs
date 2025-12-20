//! This module is deprecated as of the XSLT compiler refactoring.
//! Its functionality for parsing stylesheet definitions has been merged into the new
//! single-pass parser/builder mechanism in `parser::xslt::compiler` and `parser::xslt::parser`.
//! This file is retained to prevent build failures in modules that may still reference it,
//! but it should be removed once all dependencies are updated.

#![allow(deprecated)]

use crate::parser::ParseError;
use crate::style_types::stylesheet::Stylesheet;

#[deprecated(
    since = "0.2.0",
    note = "Functionality moved to the single-pass builder in `parser::xslt::compiler`. Use `xslt::processor::XsltParser` instead."
)]
pub struct XsltParser<'a> {
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> XsltParser<'a> {
    #[deprecated(
        since = "0.2.0",
        note = "Functionality moved to `parser::xslt::compiler::compile`"
    )]
    pub fn new(_content: &'a str) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    #[deprecated(
        since = "0.2.0",
        note = "Functionality moved to `parser::xslt::compiler::compile`. This function now returns a default Stylesheet."
    )]
    pub fn parse(self) -> Result<Stylesheet, ParseError> {
        // Return a default stylesheet to avoid breaking callers during transition.
        // A better shim would re-parse, but that is inefficient. Callers should migrate.
        Ok(Stylesheet::default())
    }
}
