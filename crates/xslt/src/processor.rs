use super::{ast, compiler};
use super::executor::{self, ExecutionError};
use petty_idf::IRNode;
use petty_style::stylesheet::Stylesheet;
use crate::error::XsltError;
use petty_template_core::{
    CompiledTemplate, DataSourceFormat, ExecutionConfig, TemplateParser,
    TemplateFeatures, TemplateFlags, TemplateError,
};
use crate::datasources::json::JsonVDocument;
use crate::datasources::xml::XmlDocument;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl From<ExecutionError> for TemplateError {
    fn from(e: ExecutionError) -> Self {
        TemplateError::ExecutionError(e.to_string())
    }
}

// A wrapper struct to implement the `CompiledTemplate` trait.
#[derive(Debug)]
pub struct XsltTemplate {
    compiled: Arc<ast::CompiledStylesheet>,
    entry_mode: Option<String>,
}

impl CompiledTemplate for XsltTemplate {
    fn execute(&self, data_source_str: &str, config: ExecutionConfig) -> Result<Vec<IRNode>, TemplateError> {
        let mut builder = super::idf_builder::IdfBuilder::new();
        match config.format {
            DataSourceFormat::Xml => {
                let doc = XmlDocument::parse(data_source_str)
                    .map_err(|e: roxmltree::Error| XsltError::from(e))?;
                let root_node = doc.root_node();
                let mut executor = executor::TemplateExecutor::new(&self.compiled, root_node, config.strict)?;
                executor.execute_with_mode(self.entry_mode.as_deref(), &mut builder)?;
            }
            DataSourceFormat::Json => {
                let json_data: serde_json::Value = serde_json::from_str(data_source_str)
                    .map_err(|e: serde_json::Error| XsltError::from(e))?;
                let doc = JsonVDocument::new(&json_data);
                let root_node = doc.root_node();
                let mut executor = executor::TemplateExecutor::new(&self.compiled, root_node, config.strict)?;
                executor.execute_with_mode(self.entry_mode.as_deref(), &mut builder)?;
            }
        }
        Ok(builder.get_result())
    }

    fn stylesheet(&self) -> Arc<Stylesheet> {
        Arc::clone(&self.compiled.stylesheet)
    }

    fn resource_base_path(&self) -> &Path {
        &self.compiled.resource_base_path
    }

    fn features(&self) -> TemplateFlags {
        self.compiled.features
    }
}

/// An implementation of `TemplateParser` for XSLT 1.0 stylesheets.
pub struct XsltParser;

impl TemplateParser for XsltParser {
    fn parse(
        &self,
        template_source: &str,
        resource_base_path: PathBuf,
    ) -> Result<TemplateFeatures, TemplateError> {
        let compiled_stylesheet = compiler::compile(template_source, resource_base_path)
            .map_err(|e: XsltError| -> TemplateError { e.into() })?;
        let compiled_arc = Arc::new(compiled_stylesheet);

        // --- Create Main Template ---
        let main_template: Arc<dyn CompiledTemplate> = Arc::new(XsltTemplate {
            compiled: Arc::clone(&compiled_arc),
            entry_mode: None, // `None` mode means start with the default template rules.
        });

        // --- Create Role Templates ---
        let mut role_templates = HashMap::new();
        for (role_name, mode_name) in &compiled_arc.role_template_modes {
            let role_template: Arc<dyn CompiledTemplate> = Arc::new(XsltTemplate {
                compiled: Arc::clone(&compiled_arc),
                entry_mode: Some(mode_name.clone()),
            });
            role_templates.insert(role_name.clone(), role_template);
        }

        Ok(TemplateFeatures {
            main_template,
            role_templates,
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use petty_idf::InlineNode;

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

    const TEST_JSON_DATA: &str = r#"{ "users": [ {"name": "Alice"}, {"name": "Bob"} ] }"#;


    #[test]
    fn test_explicit_format_selection() {
        let xslt = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
            <xsl:template match="/"><p><xsl:value-of select="users/item/name"/></p></xsl:template>
        </xsl:stylesheet>"#;

        let parser = XsltParser;
        let compiled_bundle = parser.parse(xslt, PathBuf::new()).unwrap();
        let compiled = compiled_bundle.main_template;

        // Correctly process JSON with JSON format
        let json_config = ExecutionConfig { format: DataSourceFormat::Json, ..Default::default() };
        let result_json = compiled.execute(TEST_JSON_DATA, json_config).unwrap();
        assert_eq!(result_json[0].get_text_content(), "Alice");

        // Fail to process JSON with XML format
        let xml_config = ExecutionConfig { format: DataSourceFormat::Xml, ..Default::default() };
        assert!(compiled.execute(TEST_JSON_DATA, xml_config).is_err());

        // Fail to process XML with JSON format
        let json_config_for_xml = ExecutionConfig { format: DataSourceFormat::Json, ..Default::default() };
        assert!(compiled.execute(TEST_XML_DATA_FOR_COMPLEX, json_config_for_xml).is_err());
    }

    #[test]
    fn test_strict_mode_undeclared_variable() {
        let xslt = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
            <xsl:template match="/"><p><xsl:value-of select="$undeclared"/></p></xsl:template>
        </xsl:stylesheet>"#;
        let data = "<data/>";

        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;

        // Non-strict mode: succeeds, outputs empty string
        let non_strict_config = ExecutionConfig { format: DataSourceFormat::Xml, strict: false };
        let result_non_strict = compiled.execute(data, non_strict_config).unwrap();
        assert!(result_non_strict[0].get_text_content().is_empty());

        // Strict mode: fails
        let strict_config = ExecutionConfig { format: DataSourceFormat::Xml, strict: true };
        let result_strict = compiled.execute(data, strict_config);
        assert!(result_strict.is_err());
        assert!(result_strict.unwrap_err().to_string().contains("Reference to undeclared variable: $undeclared"));
    }

    #[test]
    fn test_strict_mode_undeclared_param() {
        let xslt = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
            <xsl:template name="test"><xsl:param name="declared"/></xsl:template>
            <xsl:template match="/">
                <xsl:call-template name="test">
                    <xsl:with-param name="undeclared" select="'value'"/>
                </xsl:call-template>
            </xsl:template>
        </xsl:stylesheet>"#;
        let data = "<data/>";

        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;

        // Non-strict mode: succeeds
        let non_strict_config = ExecutionConfig { format: DataSourceFormat::Xml, strict: false };
        assert!(compiled.execute(data, non_strict_config).is_ok());

        // Strict mode: fails
        let strict_config = ExecutionConfig { format: DataSourceFormat::Xml, strict: true };
        let result_strict = compiled.execute(data, strict_config);
        assert!(result_strict.is_err());
        assert!(result_strict.unwrap_err().to_string().contains("undeclared parameter: 'undeclared'"));
    }

    #[test]
    fn test_xslt_processor_with_control_flow() {
        let parser = XsltParser;
        let compiled = parser.parse(COMPLEX_XSLT, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(TEST_XML_DATA_FOR_COMPLEX, ExecutionConfig::default()).unwrap();

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
    fn test_new_xpath_features_in_xslt() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/data">
                    <root>
                        <xsl:variable name="five" select="5"/>
                        <p>Unary minus: <xsl:value-of select="items/item[1] - $five"/></p>
                        <p>Preceding sibling: <xsl:value-of select="items/item[2]/preceding-sibling::item"/></p>
                        <p>Lang check: <xsl:value-of select="lang('en-GB')"/></p>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data xml:lang="en-GB">
                <items>
                    <item>3</item>
                    <item>10</item>
                </items>
            </data>
        "#;

        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();
        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };

        assert_eq!(root_children.len(), 3);
        assert_eq!(root_children[0].get_text_content().trim(), "Unary minus: -2");
        assert_eq!(root_children[1].get_text_content().trim(), "Preceding sibling: 3");
        assert_eq!(root_children[2].get_text_content().trim(), "Lang check: true");
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
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();

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
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();

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
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();

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
                        <p><xsl:copy-of select="'Hello World'"/></p>
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
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();

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
                        <p><xsl:copy-of select="'Hello World'"/></p>
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
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();

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
                <xsl:template match="@*|node()">
                    <xsl:copy>
                        <xsl:apply-templates select="@*|node()"/>
                    </xsl:copy>
                </xsl:template>

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
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();

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
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();
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

    #[test]
    fn test_xsl_element_avt() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template match="/">
                    <root>
                        <xsl:variable name="tag_name" select="'dynamic-tag'"/>
                        <xsl:element name="h{data/level}">
                            <xsl:value-of select="data/title"/>
                        </xsl:element>
                        <xsl:element name="{$tag_name}">
                            Content
                        </xsl:element>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <level>1</level>
                <title>Hello World</title>
            </data>
        "#;
        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();
        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        assert_eq!(root_children.len(), 2);
        // We can't easily check the tag name from the IR, but we can check the content.
        // A better test would be to have a mock OutputBuilder.
        assert_eq!(root_children[0].get_text_content().trim(), "Hello World");
        assert_eq!(root_children[1].get_text_content().trim(), "Content");
    }

    #[test]
    fn test_xslt_key_function() {
        let xslt = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:key name="user-by-id" match="user" use="@id"/>
                <xsl:key name="users-by-dept" match="user" use="dept"/>

                <xsl:template match="/">
                    <root>
                        <p>User u2: <xsl:value-of select="key('user-by-id', 'u2')/name"/></p>

                        <div id="sales-users">
                            <xsl:for-each select="key('users-by-dept', 'sales')">
                                <xsl:sort select="name"/>
                                <p><xsl:value-of select="name"/></p>
                            </xsl:for-each>
                        </div>
                    </root>
                </xsl:template>
            </xsl:stylesheet>
        "#;
        let data = r#"
            <data>
                <user id="u1"><name>Alice</name><dept>eng</dept></user>
                <user id="u2"><name>Bob</name><dept>sales</dept></user>
                <user id="u3"><name>Charlie</name><dept>eng</dept></user>
                <user id="u4"><name>David</name><dept>sales</dept></user>
            </data>
        "#;

        let parser = XsltParser;
        let compiled = parser.parse(xslt, PathBuf::new()).unwrap().main_template;
        let result_tree = compiled.execute(data, ExecutionConfig::default()).unwrap();

        let root_children = match &result_tree[0] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected root block"),
        };
        assert_eq!(root_children.len(), 2);

        // Check single key lookup
        assert_eq!(root_children[0].get_text_content().trim(), "User u2: Bob");

        // Check multi-value key lookup
        let sales_block_children = match &root_children[1] {
            IRNode::Block { children, .. } => children,
            _ => panic!("Expected sales div to be a block"),
        };
        assert_eq!(sales_block_children.len(), 2);
        assert_eq!(sales_block_children[0].get_text_content().trim(), "Bob"); // Sorted
        assert_eq!(sales_block_children[1].get_text_content().trim(), "David");
    }

    #[test]
    fn test_feature_detection_index_and_toc() {
        let xslt = r#"
            <xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:petty="https://petty.rs/ns/1.0">

                <xsl:template match="/">
                    <p>
                        <xsl:value-of select="petty:index('my-term')"/>
                    </p>
                    <toc/> <!-- Old-style TOC for feature detection -->
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let parser = XsltParser;
        let features = parser.parse(xslt, PathBuf::new()).unwrap();
        let flags = features.main_template.features();

        assert!(flags.uses_index_function, "uses_index_function should be true");
        assert!(flags.has_table_of_contents, "has_table_of_contents should be true");
        assert_eq!(features.role_templates.len(), 0);
    }

    #[test]
    fn test_xslt_role_template_parsing_and_execution() {
        let xslt_with_roles = r#"
            <xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:petty="https://petty.rs/ns/1.0">

                <!-- Main template -->
                <xsl:template match="/">
                    <p>Main Content</p>
                </xsl:template>

                <!-- Role template for page header -->
                <xsl:template match="/" petty:role="page-header">
                    <p>Page Header for page <xsl:value-of select="*/page_number"/> of <xsl:value-of select="*/page_count"/></p>
                </xsl:template>

                <!-- Role template for table of contents -->
                <xsl:template match="/" petty:role="table-of-contents">
                    <toc>
                        <xsl:for-each select="headings/item">
                            <p><xsl:value-of select="text"/> ...... <xsl:value-of select="page_number"/></p>
                        </xsl:for-each>
                    </toc>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let parser = XsltParser;
        let features = parser.parse(xslt_with_roles, PathBuf::new()).unwrap();

        // 1. Check features struct
        assert!(features.main_template.features().has_table_of_contents == false);
        assert_eq!(features.role_templates.len(), 2);
        assert!(features.role_templates.contains_key("page-header"));
        assert!(features.role_templates.contains_key("table-of-contents"));

        // 2. Execute main template
        let main_result = features.main_template.execute("<data/>", ExecutionConfig::default()).unwrap();
        assert_eq!(main_result[0].get_text_content(), "Main Content");

        // 3. Execute 'page-header' role template
        let header_template = features.role_templates.get("page-header").unwrap();
        let header_data = r#"{ "page_number": 5, "page_count": 10 }"#;
        let header_config = ExecutionConfig { format: DataSourceFormat::Json, ..Default::default() };
        let header_result = header_template.execute(header_data, header_config).unwrap();
        assert_eq!(header_result[0].get_text_content(), "Page Header for page 5 of 10");

        // 4. Execute 'table-of-contents' role template
        let toc_template = features.role_templates.get("table-of-contents").unwrap();
        let toc_data = r#"{ "headings": [ { "text": "Chapter 1", "page_number": 1 } ] }"#;
        let toc_config = ExecutionConfig { format: DataSourceFormat::Json, ..Default::default() };
        let toc_result = toc_template.execute(toc_data, toc_config).unwrap();
        assert_eq!(toc_result[0].get_text_content().trim(), "Chapter 1 ...... 1");
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
                IRNode::Paragraph { children, .. }
                | IRNode::Heading { children, .. } => {
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