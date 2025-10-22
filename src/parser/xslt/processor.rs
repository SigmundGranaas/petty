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

    #[test]
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

    #[test]
    fn test_xslt_choose_when_otherwise() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root>
                        <xsl:for-each select="data/items/item">
                            <xsl:choose>
                                <xsl:when test="@category = 'A'">
                                    <p>Category A: <xsl:value-of select="."/></p>
                                </xsl:when>
                                <xsl:when test="@category = 'B'">
                                    <p>Category B: <xsl:value-of select="."/></p>
                                </xsl:when>
                                <xsl:otherwise>
                                    <p>Other: <xsl:value-of select="."/></p>
                                </xsl:otherwise>
                            </xsl:choose>
                        </xsl:for-each>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <items>
                    <item category="B">Item 1</item>
                    <item category="C">Item 2</item>
                    <item category="A">Item 3</item>
                </items>
            </data>
        "#;

        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(data).unwrap();

        assert_eq!(result_tree.len(), 1);
        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        assert_eq!(root_children.len(), 3);
        assert_eq!(root_children[0].get_text_content().trim(), "Category B: Item 1");
        assert_eq!(root_children[1].get_text_content().trim(), "Other: Item 2");
        assert_eq!(root_children[2].get_text_content().trim(), "Category A: Item 3");
    }

    #[test]
    fn test_xslt_sort() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root>
                        <xsl:for-each select="data/items/item">
                            <xsl:sort select="name" order="ascending"/>
                            <xsl:sort select="price" data-type="number" order="descending"/>
                            <p><xsl:value-of select="name"/> - <xsl:value-of select="price"/></p>
                        </xsl:for-each>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <items>
                    <item><name>Gadget</name><price>9.99</price></item>
                    <item><name>Widget</name><price>15.50</price></item>
                    <item><name>Gadget</name><price>12.00</price></item>
                </items>
            </data>
        "#;

        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(data).unwrap();

        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        assert_eq!(root_children.len(), 3);
        // Sorted first by name (Gadget, Gadget, Widget), then by price descending within Gadget.
        assert_eq!(root_children[0].get_text_content().trim(), "Gadget - 12.00");
        assert_eq!(root_children[1].get_text_content().trim(), "Gadget - 9.99");
        assert_eq!(root_children[2].get_text_content().trim(), "Widget - 15.50");
    }

    #[test]
    fn test_xslt_attribute() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root>
                        <xsl:for-each select="data/links/link">
                            <a>
                                <xsl:attribute name="href">
                                    <xsl:value-of select="url"/>
                                </xsl:attribute>
                                <xsl:value-of select="text"/>
                            </a>
                        </xsl:for-each>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <links>
                    <link><url>https://example.com</url><text>Example</text></link>
                    <link><url>https://petty.rs</url><text>Petty</text></link>
                </links>
            </data>
        "#;
        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(data).unwrap();

        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        assert_eq!(root_children.len(), 2); // Two paragraphs, each containing one link.

        let para1_children = match &root_children[0] {
            IRNode::Paragraph { children, .. } => children,
            _ => panic!("Expected paragraph"),
        };
        let link1 = match &para1_children[0] {
            InlineNode::Hyperlink { href, children, .. } => {
                assert_eq!(href, "https://example.com");
                children[0].get_text_content()
            },
            _ => panic!("Expected hyperlink"),
        };
        assert_eq!(link1, "Example");

        let para2_children = match &root_children[1] {
            IRNode::Paragraph { children, .. } => children,
            _ => panic!("Expected paragraph"),
        };
        let link2 = match &para2_children[0] {
            InlineNode::Hyperlink { href, children, .. } => {
                assert_eq!(href, "https://petty.rs");
                children[0].get_text_content()
            },
            _ => panic!("Expected hyperlink"),
        };
        assert_eq!(link2, "Petty");
    }

    #[test]
    fn test_xslt_copy_of() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root>
                        <!-- Copy a primitive value -->
                        <p><xsl:copy-of select="'Hello World'"/></p>
                        <!-- Copy a data source fragment (the <content> node itself) -->
                        <xsl:copy-of select="data/content"/>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <content>
                    <p>This is the first paragraph.</p>
                    <div>This is a div, which becomes a block.</div>
                </content>
            </data>
        "#;
        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(data).unwrap();

        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };

        // Expect 2 children: the <p> with the literal, and a <block> for the copied <content> node
        assert_eq!(root_children.len(), 2);
        assert_eq!(root_children[0].get_text_content().trim(), "Hello World");

        // Inspect the deep-copied <content> block
        let copied_content_block = &root_children[1];
        let content_children = match copied_content_block {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected the copied node to be a block"),
        };
        assert_eq!(content_children.len(), 2);
        assert!(matches!(&content_children[0], IRNode::Paragraph { .. }));
        assert_eq!(content_children[0].get_text_content().trim(), "This is the first paragraph.");
        assert!(matches!(&content_children[1], IRNode::Block { .. }));
        assert_eq!(content_children[1].get_text_content().trim(), "This is a div, which becomes a block.");
    }

    #[test]
    fn test_xslt_copy_of_children() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root>
                        <!-- Copy a primitive value -->
                        <p><xsl:copy-of select="'Hello World'"/></p>
                        <!-- Copy the children of the <content> node -->
                        <xsl:copy-of select="data/content/*"/>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <content>
                    <p>This is the first paragraph.</p>
                    <div>This is a div, which becomes a block.</div>
                </content>
            </data>
        "#;
        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(data).unwrap();

        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        assert_eq!(root_children.len(), 3);
        assert_eq!(root_children[0].get_text_content().trim(), "Hello World");
        assert!(matches!(&root_children[1], IRNode::Paragraph { .. }));
        assert_eq!(root_children[1].get_text_content().trim(), "This is the first paragraph.");
        assert!(matches!(&root_children[2], IRNode::Block { .. }));
        assert_eq!(root_children[2].get_text_content().trim(), "This is a div, which becomes a block.");
    }

    #[test]
    fn test_xslt_copy() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <!-- Identity transform for most nodes -->
                <xsl:template match="@*|node()">
                    <xsl:copy>
                        <xsl:apply-templates select="@*|node()"/>
                    </xsl:copy>
                </xsl:template>

                <!-- Specific override for <item> -->
                <xsl:template match="item">
                    <p>Processed item: <xsl:value-of select="."/></p>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <wrapper>
                    <item>A</item>
                    <another>B</another>
                </wrapper>
            </data>
        "#;

        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(data).unwrap();

        // The top-level <data> gets copied, so we have one root node.
        assert_eq!(result_tree.len(), 1);

        let data_node_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected copied <data> to be a block")
        };
        // Inside <data> is the copied <wrapper>
        assert_eq!(data_node_children.len(), 1);

        let wrapper_node_children = match &data_node_children[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected copied <wrapper> to be a block")
        };
        // Inside <wrapper> are the processed <item> and the copied <another>
        assert_eq!(wrapper_node_children.len(), 2);

        // The <item> was transformed into a <p>
        assert!(matches!(&wrapper_node_children[0], IRNode::Paragraph { .. }));
        assert_eq!(wrapper_node_children[0].get_text_content().trim(), "Processed item: A");

        // The <another> was copied as-is
        let another_node_children = match &wrapper_node_children[1] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected copied <another> to be a block")
        };
        assert_eq!(another_node_children[0].get_text_content().trim(), "B");
    }

    #[test]
    fn test_attribute_value_template() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root>
                        <xsl:for-each select="data/links/link">
                            <a href="https://example.com/{@id}">
                               <xsl:value-of select="text"/>
                            </a>
                        </xsl:for-each>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <links>
                    <link id="123"><text>Link 1</text></link>
                </links>
            </data>
        "#;
        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap();
        let result_tree = compiled.execute(data).unwrap();
        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        let para_children = match &root_children[0] {
            IRNode::Paragraph { children, .. } => children,
            _ => panic!("Expected paragraph"),
        };
        match &para_children[0] {
            InlineNode::Hyperlink { href, .. } => {
                assert_eq!(href, "https://example.com/123");
            },
            _ => panic!("Expected hyperlink"),
        };
    }

    // Helper to get all text from an IRNode for simple assertions
    trait TestTextContent {
        fn get_text_content(&self) -> String;
    }
    impl TestTextContent for IRNode {
        fn get_text_content(&self) -> String {
            let mut s = String::new();
            match self {
                IRNode::Block { children, .. }
                | IRNode::FlexContainer { children, .. }
                | IRNode::List { children, .. }
                | IRNode::ListItem { children, .. } => {
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
                InlineNode::StyledSpan { children, .. }
                | InlineNode::Hyperlink { children, .. } => {
                    children.iter().map(|c| c.get_text_content()).collect()
                }
                _ => String::new(),
            }
        }
    }
}