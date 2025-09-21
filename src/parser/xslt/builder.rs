
use super::util::{capture_inner_xml, get_attr_owned_required, OwnedAttributes};
use crate::error::PipelineError;
use crate::idf::{IRNode, InlineNode};
use crate::xpath;
use handlebars::Handlebars;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;

// --- Pre-parsed Template AST ---

/// Represents a pre-compiled block of XSLT that can be executed repeatedly with different contexts.
#[derive(Debug, Clone)]
pub struct PreparsedTemplate(pub(super) Vec<XsltInstruction>);

/// An instruction in a pre-parsed template, representing a node or control flow statement.
#[derive(Debug, Clone)]
pub(super) enum XsltInstruction {
    // Content nodes
    StartTag {
        tag_name: Vec<u8>,
        attributes: OwnedAttributes,
    },
    EmptyTag {
        tag_name: Vec<u8>,
        attributes: OwnedAttributes,
    },
    Text(String),
    EndTag {
        tag_name: Vec<u8>,
    },
    // Control flow
    If {
        test: String,
        body: PreparsedTemplate,
    },
    ForEach {
        select: String,
        body: PreparsedTemplate,
    },
}

/// A stateful builder that constructs a single `IRNode` tree from an XML fragment.
pub struct TreeBuilder<'h> {
    pub(super) template_engine: &'h Handlebars<'static>,
    pub(super) node_stack: Vec<IRNode>,
    pub(super) inline_stack: Vec<InlineNode>,
    pub(super) row_column_index_stack: Vec<usize>,
    pub(super) is_in_table_header: bool,
}

impl<'h> TreeBuilder<'h> {
    pub fn new(template_engine: &'h Handlebars<'static>) -> Self {
        Self {
            template_engine,
            node_stack: vec![],
            inline_stack: vec![],
            row_column_index_stack: vec![],
            is_in_table_header: false,
        }
    }

    /// Public method to pre-parse an XML string into an executable template.
    pub fn preparse_from_str(&self, xml_str: &str) -> Result<PreparsedTemplate, PipelineError> {
        let wrapped_xml = format!("<petty-wrapper>{}</petty-wrapper>", xml_str);
        let mut reader = Reader::from_str(&wrapped_xml);
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        reader.read_event_into(&mut buf)?; // Consume the <petty-wrapper> start tag

        self.preparse_template(&mut reader, QName(b"petty-wrapper"))
    }

    /// Executes a pre-parsed template to build an IR tree.
    pub fn build_tree_from_preparsed(
        &mut self,
        template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<Vec<IRNode>, PipelineError> {
        // Reset state for the new sequence.
        self.node_stack.clear();
        self.inline_stack.clear();
        self.row_column_index_stack.clear();
        self.is_in_table_header = false;

        let root_node = IRNode::Root(Vec::with_capacity(16));
        self.node_stack.push(root_node);

        // Execute the main logic.
        self.execute_template(template, context)?;

        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(PipelineError::TemplateParseError(
                "Failed to construct root node.".to_string(),
            ))
        }
    }

    pub(super) fn render_text(&self, text: &str, context: &Value) -> Result<String, PipelineError> {
        if !text.contains("{{") {
            return Ok(text.to_string());
        }

        self.template_engine
            .render_template(text, context)
            .map_err(|e| PipelineError::TemplateRenderError(e.to_string()))
    }

    /// The main entry point for building a tree from an XML string.
    pub fn build_tree_from_xml_str(
        &mut self,
        xml_str: &str,
        context: &Value,
    ) -> Result<Vec<IRNode>, PipelineError> {
        self.node_stack.clear();
        self.inline_stack.clear();
        self.row_column_index_stack.clear();
        self.is_in_table_header = false;

        let wrapped_xml = format!("<petty-wrapper>{}</petty-wrapper>", xml_str);
        let mut reader = Reader::from_str(&wrapped_xml);
        reader.config_mut().trim_text(false);

        let root_node = IRNode::Root(Vec::with_capacity(16));
        self.node_stack.push(root_node);

        self.parse_nodes(&mut reader, context)?;

        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(PipelineError::TemplateParseError(
                "Failed to construct root node.".to_string(),
            ))
        }
    }

    pub(super) fn push_block_to_parent(&mut self, node: IRNode) {
        match self.node_stack.last_mut() {
            Some(IRNode::Root(children))
            | Some(IRNode::Block { children, .. })
            | Some(IRNode::FlexContainer { children, .. })
            | Some(IRNode::List { children, .. })
            | Some(IRNode::ListItem { children, .. }) => children.push(node),
            Some(IRNode::Table { header, body, .. }) => {
                let target_row = if self.is_in_table_header {
                    header.as_mut().and_then(|h| h.rows.last_mut())
                } else {
                    body.rows.last_mut()
                };
                match target_row {
                    Some(row) => match row.cells.last_mut() {
                        Some(cell) => cell.children.push(node),
                        None => log::warn!("Attempted to add node to a row with no cells."),
                    },
                    None => {
                        let section = if self.is_in_table_header {
                            "header"
                        } else {
                            "body"
                        };
                        log::warn!(
                            "Attempted to add node to a table with no rows in the current section ({}).",
                            section
                        );
                    }
                }
            }
            _ => log::warn!("Cannot add block node to current parent."),
        }
    }

    pub(super) fn push_inline_to_parent(&mut self, node: InlineNode) {
        match self.inline_stack.last_mut() {
            Some(InlineNode::StyledSpan { children, .. })
            | Some(InlineNode::Hyperlink { children, .. }) => children.push(node),
            _ => {
                if let Some(IRNode::Paragraph { children, .. }) = self.node_stack.last_mut() {
                    children.push(node);
                } else {
                    log::warn!("Cannot add inline node: not in a paragraph context.");
                }
            }
        }
    }

    /// Recursively executes a pre-parsed template AST against a given data context.
    pub(super) fn execute_template(
        &mut self,
        template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<(), PipelineError> {
        for instruction in &template.0 {
            match instruction {
                XsltInstruction::ForEach { select, body } => {
                    let selected_values = xpath::select(context, select);
                    let items: Vec<&Value> =
                        if let Some(arr) = selected_values.first().and_then(|v| v.as_array()) {
                            arr.iter().collect()
                        } else {
                            selected_values
                        };
                    for item_context in items {
                        self.execute_template(body, item_context)?;
                    }
                }
                XsltInstruction::If { test, body } => {
                    let results = xpath::select(context, test);
                    let is_truthy = !results.is_empty()
                        && results
                        .iter()
                        .all(|v| !v.is_null() && v.as_bool() != Some(false));
                    if is_truthy {
                        self.execute_template(body, context)?;
                    }
                }
                XsltInstruction::StartTag {
                    tag_name,
                    attributes,
                } => match tag_name.as_slice() {
                    b"container" | b"list" | b"list-item" | b"flex-container" | b"text" => {
                        self.handle_block_element(tag_name, attributes)?
                    }
                    b"link" => self.handle_link(context, attributes)?,
                    b"strong" | b"b" | b"em" | b"i" => {
                        self.handle_styled_span(tag_name, attributes)?
                    }
                    b"header" => self.handle_header_start()?,
                    b"tbody" => {} // tbody is a semantic wrapper, no action needed.
                    b"row" => self.handle_row_start()?,
                    b"cell" => self.handle_cell_start(attributes)?,
                    _ => {
                        log::warn!(
                            "Ignoring unknown start tag during template execution: {}",
                            String::from_utf8_lossy(tag_name)
                        );
                    }
                },
                XsltInstruction::EmptyTag {
                    tag_name,
                    attributes,
                } => match tag_name.as_slice() {
                    b"xsl:value-of" => self.handle_value_of(attributes, context)?,
                    b"image" => self.handle_image(attributes, context)?,
                    b"br" => self.handle_line_break(),
                    _ => {}
                },
                XsltInstruction::Text(text) => {
                    // FIX: This is the critical change to prevent stack overflow.
                    // If we find a captured table, we parse it directly with the low-level
                    // parser instead of recursively calling the high-level one.
                    if text.trim().starts_with("<table") {
                        let mut reader = Reader::from_str(text);
                        reader.config_mut().trim_text(false);
                        // Use the current builder and the direct parsing method.
                        self.parse_nodes(&mut reader, context)?;
                    } else if !text.trim().is_empty() {
                        let rendered_text = if text.contains("{{") {
                            self.render_text(text, context)?
                        } else {
                            text.clone()
                        };
                        self.push_inline_to_parent(InlineNode::Text(rendered_text));
                    }
                }
                XsltInstruction::EndTag { tag_name } => {
                    self.handle_end_tag(QName(tag_name))?;
                }
            }
        }
        Ok(())
    }

    /// Recursively parses an XML stream into a preparsed template AST.
    pub(super) fn preparse_template(
        &self,
        reader: &mut Reader<&[u8]>,
        end_qname: QName,
    ) -> Result<PreparsedTemplate, PipelineError> {
        let mut instructions = Vec::new();
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => {
                    let attributes = e
                        .attributes()
                        .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
                        .collect::<Result<OwnedAttributes, _>>()?;
                    let name = e.name();
                    match name.as_ref() {
                        b"xsl:for-each" => {
                            let select =
                                get_attr_owned_required(&attributes, b"select", b"xsl:for-each")?;
                            let body = self.preparse_template(reader, name)?;
                            instructions.push(XsltInstruction::ForEach { select, body });
                        }
                        b"xsl:if" => {
                            let test = get_attr_owned_required(&attributes, b"test", b"xsl:if")?;
                            let body = self.preparse_template(reader, name)?;
                            instructions.push(XsltInstruction::If { test, body });
                        }
                        b"table" => {
                            let inner_xml =
                                capture_inner_xml(reader, name)?;
                            let attributes_str = e
                                .attributes()
                                .flatten()
                                .map(|a| {
                                    format!(
                                        " {}=\"{}\"",
                                        String::from_utf8_lossy(a.key.as_ref()),
                                        a.unescape_value().unwrap_or_default()
                                    )
                                })
                                .collect::<String>();

                            let full_tag =
                                format!("<table{}>{}</table>", attributes_str, inner_xml);
                            instructions.push(XsltInstruction::Text(full_tag));
                        }
                        _ => instructions.push(XsltInstruction::StartTag {
                            tag_name: name.as_ref().to_vec(),
                            attributes,
                        }),
                    }
                }
                XmlEvent::Empty(e) => {
                    let attributes = e
                        .attributes()
                        .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
                        .collect::<Result<OwnedAttributes, _>>()?;
                    instructions.push(XsltInstruction::EmptyTag {
                        tag_name: e.name().as_ref().to_vec(),
                        attributes,
                    });
                }
                XmlEvent::Text(e) => {
                    instructions.push(XsltInstruction::Text(e.unescape()?.into_owned()));
                }
                XmlEvent::End(e) => {
                    if e.name() == end_qname {
                        break;
                    }
                    instructions.push(XsltInstruction::EndTag {
                        tag_name: e.name().as_ref().to_vec(),
                    });
                }
                XmlEvent::Eof => {
                    if end_qname.as_ref() != b"petty-wrapper" {
                        return Err(PipelineError::TemplateParseError(format!(
                            "Unexpected EOF while preparsing template expecting end tag </{}>",
                            String::from_utf8_lossy(end_qname.as_ref())
                        )));
                    }
                    break;
                }
                _ => (),
            }
            buf.clear();
        }
        Ok(PreparsedTemplate(instructions))
    }

    pub(super) fn parse_nodes(
        &mut self,
        reader: &mut Reader<&[u8]>,
        context: &Value,
    ) -> Result<(), PipelineError> {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => self.handle_start_tag(&e, reader, context)?,
                XmlEvent::Empty(e) => self.handle_empty_tag(&e, context)?,
                XmlEvent::Text(e) => {
                    let text = e.unescape()?;
                    if !text.trim().is_empty() {
                        let rendered_text = if text.contains("{{") {
                            self.render_text(&text, context)?
                        } else {
                            text.into_owned()
                        };
                        self.push_inline_to_parent(InlineNode::Text(rendered_text));
                    }
                }
                XmlEvent::End(e) => {
                    if self.handle_end_tag(e.name())? {
                        return Ok(());
                    }
                }
                XmlEvent::Eof => return Ok(()),
                _ => (),
            }
            buf.clear();
        }
    }

    fn handle_empty_tag(
        &mut self,
        e: &BytesStart,
        context: &Value,
    ) -> Result<(), PipelineError> {
        let attributes = e
            .attributes()
            .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
            .collect::<Result<OwnedAttributes, _>>()?;

        match e.name().as_ref() {
            b"xsl:value-of" => self.handle_value_of(&attributes, context)?,
            b"image" => self.handle_image(&attributes, context)?,
            b"br" => self.handle_line_break(),
            b"rectangle" | b"page-break" => {}
            _ => {}
        }
        Ok(())
    }

    fn handle_start_tag(
        &mut self,
        e: &BytesStart,
        reader: &mut Reader<&[u8]>,
        context: &Value,
    ) -> Result<(), PipelineError> {
        let attributes = e
            .attributes()
            .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
            .collect::<Result<OwnedAttributes, _>>()?;

        match e.name().as_ref() {
            b"xsl:for-each" => self.handle_for_each(e, reader, context, &attributes)?,
            b"xsl:if" => self.handle_if(e, reader, context, &attributes)?,
            b"container" | b"list" | b"list-item" | b"flex-container" | b"text" => {
                self.handle_block_element(e.name().as_ref(), &attributes)?
            }
            b"link" => self.handle_link(context, &attributes)?,
            b"strong" | b"b" | b"em" | b"i" => {
                self.handle_styled_span(e.name().as_ref(), &attributes)?
            }
            b"table" => self.handle_table_start(&attributes, reader, context)?,
            b"header" => self.handle_header_start()?,
            b"tbody" => {}
            b"row" => self.handle_row_start()?,
            b"cell" => self.handle_cell_start(&attributes)?,
            b"petty-wrapper" => {}
            b"columns" | b"column" => {
                reader.read_to_end_into(e.name(), &mut vec![])?;
            }
            _ => {
                log::warn!(
                    "Ignoring unknown start tag: {}",
                    String::from_utf8_lossy(e.name().as_ref())
                );
            }
        }
        Ok(())
    }

    fn handle_end_tag(&mut self, qname: QName) -> Result<bool, PipelineError> {
        let tag_name = qname.as_ref();
        match tag_name {
            b"container" | b"list" | b"list-item" | b"text" | b"flex-container" => {
                if let Some(node) = self.node_stack.pop() {
                    self.push_block_to_parent(node);
                }
            }
            b"table" => {}
            b"row" => {
                self.row_column_index_stack.pop();
            }
            b"cell" => {
                if let Some(idx) = self.row_column_index_stack.last_mut() {
                    *idx += 1;
                }
            }
            b"header" => self.is_in_table_header = false,
            b"tbody" | b"xsl:if" | b"xsl:for-each" => {}
            b"link" | b"strong" | b"b" | b"em" | b"i" => {
                if let Some(node) = self.inline_stack.pop() {
                    self.push_inline_to_parent(node);
                }
            }
            b"petty-wrapper" => return Ok(true),
            _ => {}
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::{InlineNode, IRNode};
    use crate::stylesheet::Color;
    use handlebars::Handlebars;
    use serde_json::json;

    // MODIFIED: Test helper now creates a mutable builder.
    fn build_test_tree(xml: &str, data: &Value) -> Result<Vec<IRNode>, PipelineError> {
        let handlebars = Handlebars::new();
        let mut builder = TreeBuilder::new(&handlebars);
        builder.build_tree_from_xml_str(xml, data)
    }

    #[test]
    fn test_inline_style_override() {
        let data = json!({});
        let template = r##"<text font-size="20pt" color="#ff0000">Red Text</text>"##;
        let tree = build_test_tree(template, &data).unwrap();
        assert_eq!(tree.len(), 1);
        if let IRNode::Paragraph {
            style_override, ..
        } = &tree[0]
        {
            let override_style = style_override
                .as_ref()
                .expect("Should have style override");
            assert_eq!(override_style.font_size, Some(20.0));
            assert_eq!(
                override_style.color,
                Some(Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 1.0
                })
            );
        } else {
            panic!("Expected a Paragraph node");
        }
    }

    #[test]
    fn test_for_each_loop_processes_all_items() {
        let data = json!({
            "items": [
                { "name": "Apple" },
                { "name": "Banana" },
                { "name": "Cherry" }
            ]
        });
        let template = r#"
            <xsl:for-each select="items">
                <text><xsl:value-of select="name"/></text>
            </xsl:for-each>
        "#;

        let tree = build_test_tree(template, &data).unwrap();

        assert_eq!(tree.len(), 3, "Should create a node for each item in the loop");

        let content: Vec<String> = tree
            .iter()
            .map(|node| {
                if let IRNode::Paragraph { children, .. } = node {
                    if let Some(InlineNode::Text(text)) = children.first() {
                        return text.clone();
                    }
                }
                panic!("Expected a paragraph with text");
            })
            .collect();

        assert_eq!(content, vec!["Apple", "Banana", "Cherry"]);
    }

    #[test]
    fn test_block_elements_are_nested_correctly() {
        let data = json!({});
        let template = r#"
            <text style="p1">Paragraph 1</text>
            <container style="c1">
                <text style="p2">Paragraph 2</text>
            </container>
            <text style="p3">Paragraph 3</text>
        "#;
        let tree = build_test_tree(template, &data).unwrap();

        assert_eq!(tree.len(), 3);

        let p1 = &tree[0];
        assert!(matches!(p1, IRNode::Paragraph { style_name, .. } if style_name == &Some("p1".to_string())));

        let c1 = &tree[1];
        if let IRNode::Block {
            style_name,
            children,
            ..
        } = c1
        {
            assert_eq!(style_name, &Some("c1".to_string()));
            assert_eq!(children.len(), 1);
            let p2 = &children[0];
            assert!(matches!(p2, IRNode::Paragraph { style_name, .. } if style_name == &Some("p2".to_string())));
        } else {
            panic!("Expected a Block node, got {:?}", c1);
        }

        let p3 = &tree[2];
        assert!(matches!(p3, IRNode::Paragraph { style_name, .. } if style_name == &Some("p3".to_string())));
    }

    fn get_text_from_node(node: &IRNode) -> String {
        if let IRNode::Paragraph { children, .. } = node {
            if let Some(InlineNode::Text(text)) = children.first() {
                return text.clone();
            }
        }
        String::new()
    }

    #[test]
    fn test_table_with_header_and_body_builds_correctly() {
        let data = json!({
            "items": [
                { "name": "Item 1" },
                { "name": "Item 2" }
            ]
        });
        let template = r#"
            <table margin="20pt">
                <columns><column /></columns>
                <header>
                    <row><cell><text>Header</text></cell></row>
                </header>
                <tbody>
                    <xsl:for-each select="items">
                        <row><cell><text><xsl:value-of select="name"/></text></cell></row>
                    </xsl:for-each>
                </tbody>
            </table>
        "#;

        let tree = build_test_tree(template, &data).unwrap();
        assert_eq!(tree.len(), 1);

        if let Some(IRNode::Table {
                        header,
                        body,
                        style_override,
                        ..
                    }) = tree.first()
        {
            let override_style = style_override
                .as_ref()
                .expect("Table should have an override style");
            assert_eq!(override_style.margin.as_ref().unwrap().top, 20.0);

            let header_node = header.as_ref().expect("Table should have a header");
            assert_eq!(header_node.rows.len(), 1, "Header should have one row");
            let header_cell = &header_node.rows[0].cells[0];
            assert_eq!(header_cell.children.len(), 1);
            assert_eq!(get_text_from_node(&header_cell.children[0]), "Header");

            assert_eq!(
                body.rows.len(),
                2,
                "Table body should have two rows from the for-each loop"
            );
            let body_cell_1 = &body.rows[0].cells[0];
            assert_eq!(body_cell_1.children.len(), 1);
            assert_eq!(get_text_from_node(&body_cell_1.children[0]), "Item 1");

            let body_cell_2 = &body.rows[1].cells[0];
            assert_eq!(body_cell_2.children.len(), 1);
            assert_eq!(get_text_from_node(&body_cell_2.children[0]), "Item 2");
        } else {
            panic!("Expected an IRNode::Table");
        }
    }
}