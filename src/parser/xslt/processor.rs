// FILE: src/parser/xslt/processor.rs
use super::ast::CompiledStylesheet;
use super::compiler;
use super::executor;
use crate::core::idf::IRNode;
use crate::core::style::stylesheet::Stylesheet;
use crate::error::PipelineError;
use crate::parser::json_ds::JsonVDocument;
use crate::parser::processor::{CompiledTemplate, TemplateParser};
use crate::parser::xml::XmlDocument;
use crate::parser::ParseError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// --- The Compiled Artifact ---

impl CompiledTemplate for CompiledStylesheet {
    fn execute(&self, data_source_str: &str) -> Result<Vec<IRNode>, PipelineError> {
        // Attempt to parse as JSON first. If it succeeds, use the JSON VDOM.
        if let Ok(json_data) = serde_json::from_str(data_source_str) {
            let doc = JsonVDocument::new(&json_data);
            let root_node = doc.root_node();
            let mut executor = executor::TemplateExecutor::new(self, root_node);
            let ir_nodes = executor.build_tree()?;
            Ok(ir_nodes)
        } else {
            // Fall back to parsing as XML.
            let doc = XmlDocument::parse(data_source_str).map_err(ParseError::from)?;
            let root_node = doc.root_node();
            let mut executor = executor::TemplateExecutor::new(self, root_node);
            let ir_nodes = executor.build_tree()?;
            Ok(ir_nodes)
        }
    }

    fn stylesheet(&self) -> &Stylesheet {
        &self.stylesheet
    }

    fn resource_base_path(&self) -> &Path {
        &self.resource_base_path
    }
}

// --- The Parser ---

pub struct XsltParser;

impl TemplateParser for XsltParser {
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<Arc<dyn CompiledTemplate>, ParseError> {
        // The new compiler entry point uses the builder pattern internally.
        // NOTE: The new compiler is a work-in-progress. Full feature support is not guaranteed.
        let compiled_stylesheet =
            compiler::compile(template_source, resource_base_path)?;
        Ok(Arc::new(compiled_stylesheet))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::idf::InlineNode;

    const COMPLEX_XSLT: &str = r#"
        <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:fo="http://www.w3.org/1999/XSL/Format">

            <xsl:template name="user-details">
                <xsl:param name="user-node"/>
                <xsl:param name="prefix" select="'User: '"/>
                <p>
                    <xsl:value-of select="$prefix"/>
                    <xsl:value-of select="$user-node/name"/>
                </p>
            </xsl:template>

            <xsl:template match="/">
                <fo:block>
                    <xsl:for-each select="data/users/user">
                        <xsl:if test="@status = 'active'">
                           <xsl:call-template name="user-details">
                               <xsl:with-param name="user-node" select="."/>
                           </xsl:call-template>
                        </xsl:if>
                    </xsl:for-each>
                </fo:block>
            </xsl:template>

        </xsl:stylesheet>
    "#;

    const TEST_XML_DATA_FOR_COMPLEX: &str = r#"
        <data>
            <users>
                <user id="u1" status="active"><name>Alice</name></user>
                <user id="u2" status="inactive"><name>Bob</name></user>
                <user id="u3" status="active"><name>Charlie</name></user>
            </users>
        </data>
    "#;

    // NOTE: The new compiler is a work-in-progress. These tests are aspirational and may fail
    // until the placeholder logic in compiler.rs is fully implemented.
    #[test]
    #[ignore] // Ignoring because the compiler is not fully implemented yet.
    fn test_xslt_processor_with_control_flow() {
        let parser = XsltParser;
        let compiled = parser.parse(COMPLEX_XSLT, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(TEST_XML_DATA_FOR_COMPLEX).unwrap();

        assert_eq!(result_tree.len(), 1, "Expected a single root block node");
        let root_block = &result_tree[0];

        let children = match root_block {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected IRNode::Block"),
        };
        // Expect 2 paragraphs: Alice, Charlie
        assert_eq!(children.len(), 2, "Expected two paragraph nodes");

        // Check text content
        assert_eq!(children[0].get_text_content().trim(), "User: Alice");
        assert_eq!(children[1].get_text_content().trim(), "User: Charlie");
    }

    // Helper to get all text from an IRNode for simple assertions
    trait TestTextContent {
        fn get_text_content(&self) -> String;
    }
    impl TestTextContent for IRNode {
        fn get_text_content(&self) -> String {
            let mut s = String::new();
            match self {
                IRNode::Block { children, .. } | IRNode::FlexContainer { children, .. } | IRNode::List { children, .. } | IRNode::ListItem { children, .. } => {
                    for child in children {
                        s.push_str(&child.get_text_content());
                    }
                }
                IRNode::Paragraph { children, .. } => {
                    for inline in children {
                        s.push_str(&inline.get_text_content());
                    }
                }
                _ => {}
            }
            s
        }
    }
    impl TestTextContent for InlineNode {
        fn get_text_content(&self) -> String {
            match self {
                InlineNode::Text(t) => t.clone(),
                InlineNode::StyledSpan { children, .. } | InlineNode::Hyperlink { children, .. } => {
                    children.iter().map(|c| c.get_text_content()).collect()
                }
                _ => String::new(),
            }
        }
    }
}