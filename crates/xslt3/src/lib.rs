//! # petty-xslt3
//!
//! XSLT 3.0 processor for the Petty PDF engine.
//!
//! This crate provides a comprehensive implementation of the [XSLT 3.0 specification](https://www.w3.org/TR/xslt-30/)
//! with full [XPath 3.1](https://www.w3.org/TR/xpath-31/) expression support. It extends the XSLT 1.0
//! foundation with modern features for streaming, error handling, and data structures.
//!
//! ## Features
//!
//! ### Core XSLT 3.0 Instructions
//! - **Text Value Templates**: Dynamic text content with `expand-text="yes"` and `{$expr}` syntax
//! - **Error Handling**: `xsl:try`/`xsl:catch` for graceful error recovery
//! - **Iteration**: `xsl:iterate` with `xsl:break` and `xsl:next-iteration` for streaming iteration
//! - **Grouping**: `xsl:for-each-group` with `group-by`, `group-adjacent`, `group-starting-with`, `group-ending-with`
//! - **String Processing**: `xsl:analyze-string` for regex-based text processing with match groups
//! - **Data Structures**: `xsl:map`, `xsl:array` construction and manipulation
//! - **Parallel Processing**: `xsl:fork` for parallel branch execution
//! - **Merge Processing**: `xsl:merge` for combining sorted sequences
//! - **Dynamic Evaluation**: `xsl:evaluate` for runtime XPath execution
//!
//! ### Streaming Support
//! - **Accumulators**: Stateful processing during streaming with `xsl:accumulator`
//! - **Streamability Analysis**: Automatic detection of streamable expressions (Posture/Sweep)
//! - **Event-Driven Processing**: Memory-efficient processing of large documents
//!
//! ### Package Support
//! - **Modularization**: `xsl:package`, `xsl:use-package` for stylesheet reuse
//! - **Visibility Control**: `public`, `private`, `final` component visibility
//! - **Overrides**: Component overriding via `xsl:override`
//!
//! ## Quick Start
//!
//! ### Simple Transformation
//!
//! ```rust,ignore
//! use petty_xslt3::execute_xslt3;
//!
//! let xslt = r#"
//!     <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
//!         <xsl:template match="/">
//!             <result><xsl:value-of select="//name"/></result>
//!         </xsl:template>
//!     </xsl:stylesheet>
//! "#;
//!
//! let xml = "<root><name>Hello World</name></root>";
//! let result = execute_xslt3(xslt, xml)?;
//! ```
//!
//! ### Using the Compiler and Executor
//!
//! ```rust,ignore
//! use petty_xslt3::{compile_stylesheet, TemplateExecutor3};
//! use petty_xslt::datasources::xml::XmlDocument;
//!
//! // 1. Compile the stylesheet
//! let stylesheet = compile_stylesheet(xslt_source)?;
//!
//! // 2. Parse the XML data
//! let doc = XmlDocument::parse(xml_data)?;
//!
//! // 3. Execute the transformation
//! let mut executor = TemplateExecutor3::new(&stylesheet, doc.root_node(), false)?;
//! let ir_nodes = executor.build_tree()?;
//! ```
//!
//! ## Architecture
//!
//! The crate follows a **Compiler-Executor** architecture:
//!
//! 1. **Compilation Phase** ([`CompilerBuilder3`]): Parses XSLT source into a [`CompiledStylesheet3`]
//!    containing templates, functions, variables, and metadata.
//!
//! 2. **Execution Phase** ([`TemplateExecutor3`]): Processes input data against the compiled
//!    stylesheet, producing Intermediate Representation (IR) nodes for rendering.
//!
//! ## Key Types
//!
//! | Type | Description |
//! |------|-------------|
//! | [`CompiledStylesheet3`] | Compiled XSLT 3.0 stylesheet containing all declarations |
//! | [`CompilerBuilder3`] | Event-driven stylesheet compiler (SAX-style) |
//! | [`TemplateExecutor3`] | Template execution engine with variable/accumulator state |
//! | [`Xslt3Instruction`] | AST enum representing all XSLT 3.0 instructions (~50 variants) |
//! | [`Xslt3Error`] | Unified error type for compilation and runtime errors |
//! | [`Xslt3Parser`] | High-level parser implementing `TemplateParser` trait |
//!
//! ## Modules
//!
//! - [`ast`]: Abstract syntax tree types for XSLT 3.0 instructions
//! - [`compiler`]: Event-driven stylesheet compilation
//! - [`executor`]: Template execution engine
//! - [`streaming`]: Streaming execution and streamability analysis
//! - [`packages`]: XSLT 3.0 package support
//! - [`resolver`]: Import/include resolution with caching
//! - [`error`]: Error types and handling
//!
//! ## XPath 3.1 Integration
//!
//! This crate uses [`petty_xpath31`] for expression evaluation, providing:
//! - Maps and arrays (`map { }`, `[ ]`)
//! - Arrow operator (`=>`)
//! - String concatenation (`||`)
//! - Higher-order functions (`fold-left`, `filter`, `for-each`)
//! - Let expressions (`let $x := ... return ...`)
//!
//! ## See Also
//!
//! - [`petty_xslt`]: XSLT 1.0 foundation crate
//! - [`petty_xpath31`]: XPath 3.1 expression evaluator
//! - [`petty_idf`]: Intermediate Document Format for rendering

pub mod ast;
pub mod compiler;
pub mod error;
pub mod executor;
pub mod packages;
pub mod processor;
pub mod resolver;
pub mod streaming;

mod compiler_handlers;
mod executor_handlers;

pub use ast::{CompiledStylesheet3, PreparsedTemplate, Xslt3Features, Xslt3Instruction};
pub use compiler::{CompilerBuilder3, StylesheetBuilder3};
pub use error::Xslt3Error;
pub use executor::{ExecutionError, TemplateExecutor3};
pub use processor::{Xslt3Parser, XsltVersion, detect_xslt_version};
pub use resolver::{CachingStylesheetResolver, StylesheetResolver, compile_stylesheet};

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "testing"))]
pub mod test_helpers {
    use crate::error::Xslt3Error;
    use crate::executor::TemplateExecutor3;
    use crate::resolver::compile_stylesheet;
    use petty_idf::{IRNode, InlineNode};
    use petty_xslt::datasources::xml::XmlDocument;

    pub fn parse_stylesheet(
        xslt_source: &str,
    ) -> Result<crate::ast::CompiledStylesheet3, Xslt3Error> {
        compile_stylesheet(xslt_source)
    }

    pub fn execute_xslt3(xslt_source: &str, xml_data: &str) -> Result<Vec<IRNode>, Xslt3Error> {
        let stylesheet = parse_stylesheet(xslt_source)?;
        let doc = XmlDocument::parse(xml_data)
            .map_err(|e| Xslt3Error::runtime(format!("XML parse error: {}", e)))?;
        let root_node = doc.root_node();
        let mut executor = TemplateExecutor3::new(&stylesheet, root_node, false)?;
        executor
            .build_tree()
            .map_err(|e| Xslt3Error::runtime(e.to_string()))
    }

    pub fn get_text_content(nodes: &[IRNode]) -> String {
        nodes
            .iter()
            .map(node_text_content)
            .collect::<Vec<_>>()
            .join("")
    }

    fn node_text_content(node: &IRNode) -> String {
        let mut s = String::new();
        match node {
            IRNode::Root(children) => {
                for child in children {
                    s.push_str(&node_text_content(child));
                }
            }
            IRNode::Block { children, .. }
            | IRNode::FlexContainer { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => {
                for child in children {
                    s.push_str(&node_text_content(child));
                }
            }
            IRNode::Paragraph { children, .. } | IRNode::Heading { children, .. } => {
                for inline in children {
                    s.push_str(&inline_text_content(inline));
                }
            }
            _ => {}
        }
        s
    }

    fn inline_text_content(node: &InlineNode) -> String {
        match node {
            InlineNode::Text(t) => t.clone(),
            InlineNode::StyledSpan { children, .. } | InlineNode::Hyperlink { children, .. } => {
                children.iter().map(inline_text_content).collect()
            }
            _ => String::new(),
        }
    }

    pub fn result_to_string(nodes: &[IRNode]) -> String {
        nodes
            .iter()
            .map(node_to_string)
            .collect::<Vec<_>>()
            .join("")
    }

    fn node_to_string(node: &IRNode) -> String {
        match node {
            IRNode::Root(children) => children
                .iter()
                .map(node_to_string)
                .collect::<Vec<_>>()
                .join(""),
            IRNode::Block { meta, children, .. } => {
                let id_attr = meta
                    .id
                    .as_ref()
                    .map_or(String::new(), |id| format!(" id=\"{}\"", id));
                let inner = children
                    .iter()
                    .map(node_to_string)
                    .collect::<Vec<_>>()
                    .join("");
                format!("<block{}>{}</block>", id_attr, inner)
            }
            IRNode::FlexContainer { children, .. }
            | IRNode::List { children, .. }
            | IRNode::ListItem { children, .. } => children
                .iter()
                .map(node_to_string)
                .collect::<Vec<_>>()
                .join(""),
            IRNode::Paragraph { children, .. } | IRNode::Heading { children, .. } => children
                .iter()
                .map(inline_to_string)
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        }
    }

    fn inline_to_string(node: &InlineNode) -> String {
        match node {
            InlineNode::Text(t) => t.clone(),
            InlineNode::StyledSpan { children, .. } => {
                children.iter().map(inline_to_string).collect()
            }
            InlineNode::Hyperlink { href, children, .. } => {
                let inner: String = children.iter().map(inline_to_string).collect();
                format!("<a href=\"{}\">{}</a>", href, inner)
            }
            _ => String::new(),
        }
    }

    pub fn find_hyperlinks(nodes: &[IRNode]) -> Vec<(String, String)> {
        let mut result = Vec::new();
        collect_hyperlinks(nodes, &mut result);
        result
    }

    fn collect_hyperlinks(nodes: &[IRNode], result: &mut Vec<(String, String)>) {
        for node in nodes {
            match node {
                IRNode::Root(children)
                | IRNode::Block { children, .. }
                | IRNode::FlexContainer { children, .. }
                | IRNode::List { children, .. }
                | IRNode::ListItem { children, .. } => {
                    collect_hyperlinks(children, result);
                }
                IRNode::Paragraph { children, .. } | IRNode::Heading { children, .. } => {
                    collect_hyperlinks_inline(children, result);
                }
                _ => {}
            }
        }
    }

    fn collect_hyperlinks_inline(inlines: &[InlineNode], result: &mut Vec<(String, String)>) {
        for node in inlines {
            match node {
                InlineNode::Hyperlink { href, children, .. } => {
                    let text: String = children.iter().map(inline_text_content).collect();
                    result.push((href.clone(), text));
                }
                InlineNode::StyledSpan { children, .. } => {
                    collect_hyperlinks_inline(children, result);
                }
                _ => {}
            }
        }
    }

    pub fn find_ids(nodes: &[IRNode]) -> Vec<String> {
        let mut result = Vec::new();
        collect_ids(nodes, &mut result);
        result
    }

    fn collect_ids(nodes: &[IRNode], result: &mut Vec<String>) {
        for node in nodes {
            if let Some(meta) = node.meta()
                && let Some(id) = &meta.id
            {
                result.push(id.clone());
            }
            match node {
                IRNode::Root(children)
                | IRNode::Block { children, .. }
                | IRNode::FlexContainer { children, .. }
                | IRNode::List { children, .. }
                | IRNode::ListItem { children, .. } => {
                    collect_ids(children, result);
                }
                _ => {}
            }
        }
    }
}
