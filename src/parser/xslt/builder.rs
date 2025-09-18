// src/parser/xslt/builder.rs
// src/parser/xslt/builder.rs
use super::util::OwnedAttributes;
use crate::error::PipelineError;
use crate::idf::{IRNode, InlineNode};
use handlebars::Handlebars;
use log::debug;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;

/// A stateful builder that constructs a single `IRNode` tree from an XML fragment.
pub(super) struct TreeBuilder<'a> {
    pub(super) template_engine: &'a Handlebars<'static>,
    // A stack of nodes currently being built. The top of the stack is the current parent.
    pub(super) node_stack: Vec<IRNode>,
    // A stack for inline nodes (e.g., within a paragraph).
    pub(super) inline_stack: Vec<InlineNode>,
    // Tracks the current column index when parsing a table row.
    pub(super) row_column_index_stack: Vec<usize>,
    // Tracks if the parser is currently inside a <table>'s <header> tag.
    pub(super) is_in_table_header: bool,
}

impl<'a> TreeBuilder<'a> {
    pub(super) fn new(template_engine: &'a Handlebars<'static>) -> Self {
        Self {
            template_engine,
            node_stack: vec![],
            inline_stack: vec![],
            row_column_index_stack: vec![],
            is_in_table_header: false,
        }
    }

    /// The main entry point for building a tree from an XML string.
    pub(super) fn build_tree_from_xml_str(
        &mut self,
        xml_str: &str,
        context: &'a Value,
    ) -> Result<Vec<IRNode>, PipelineError> {
        // Wrap the fragment in a dummy root to ensure parsing terminates correctly.
        let wrapped_xml = format!("<petty-wrapper>{}</petty-wrapper>", xml_str);
        let mut reader = Reader::from_str(&wrapped_xml);
        reader.config_mut().trim_text(false);

        let root_node = IRNode::Root(vec![]);
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

    /// Adds a constructed block-level node to its parent in the tree.
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
                        let section = if self.is_in_table_header { "header" } else { "body" };
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

    /// Adds a constructed inline-level node to its parent in the tree.
    pub(super) fn push_inline_to_parent(&mut self, node: InlineNode) {
        match self.inline_stack.last_mut() {
            Some(InlineNode::StyledSpan { children, .. })
            | Some(InlineNode::Hyperlink { children, .. }) => children.push(node),
            _ => {
                // If there's no inline parent, add to the paragraph at the top of the block stack
                if let Some(IRNode::Paragraph { children, .. }) = self.node_stack.last_mut() {
                    children.push(node);
                } else {
                    log::warn!("Cannot add inline node: not in a paragraph context.");
                }
            }
        }
    }

    /// Iteratively parses XML nodes and builds the `IRNode` tree.
    pub(super) fn parse_nodes(
        &mut self,
        reader: &mut Reader<&[u8]>,
        context: &'a Value,
    ) -> Result<(), PipelineError> {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => self.handle_start_tag(&e, reader, context)?,
                XmlEvent::Empty(e) => self.handle_empty_tag(&e, context)?,
                XmlEvent::Text(e) => {
                    let text = e.unescape()?;
                    if !text.trim().is_empty() {
                        let rendered_text = self
                            .template_engine
                            .render_template(&text, context)
                            .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?;
                        self.push_inline_to_parent(InlineNode::Text(rendered_text));
                    }
                }
                XmlEvent::End(e) => {
                    if self.handle_end_tag(e.name())? {
                        return Ok(()); // Reached the end tag for this parsing context
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
        context: &'a Value,
    ) -> Result<(), PipelineError> {
        debug!(
            "Handling empty tag: <{}>",
            String::from_utf8_lossy(e.name().as_ref())
        );
        match e.name().as_ref() {
            b"xsl:value-of" => self.handle_value_of(e, context)?,
            b"image" => self.handle_image(e, context)?,
            b"br" => self.handle_line_break(),
            b"rectangle" | b"page-break" => { /* Layout-specific, ignored in IR */ }
            _ => {}
        }
        Ok(())
    }

    fn handle_start_tag(
        &mut self,
        e: &BytesStart,
        reader: &mut Reader<&[u8]>,
        context: &'a Value,
    ) -> Result<(), PipelineError> {
        debug!(
            "Handling start tag: <{}>",
            String::from_utf8_lossy(e.name().as_ref())
        );

        let attributes = e
            .attributes()
            .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
            .collect::<Result<OwnedAttributes, _>>()?;

        match e.name().as_ref() {
            // --- Control Flow ---
            b"xsl:for-each" => self.handle_for_each(e, reader, context, &attributes)?,
            b"xsl:if" => self.handle_if(e, reader, context, &attributes)?,

            // --- Block Elements ---
            b"container" | b"list" | b"list-item" | b"flex-container" | b"text" => {
                self.handle_block_element(e, &attributes)?
            }

            // --- Inline Elements ---
            b"link" => self.handle_link(context, &attributes)?,
            b"strong" | b"b" | b"em" | b"i" => self.handle_styled_span(e.name().as_ref(), &attributes)?,

            // --- Tables ---
            b"table" => self.handle_table_start(&attributes, reader, context)?,
            b"header" => self.handle_header_start()?,
            b"tbody" => {} // tbody is implicit, just parse children
            b"row" => self.handle_row_start()?,
            b"cell" => self.handle_cell_start(&attributes)?,

            // --- Structural / Ignored ---
            b"petty-wrapper" => {} // Descend into dummy wrapper
            b"columns" | b"column" => {
                // <columns> is handled inside handle_table_start, so we skip it here.
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

    /// Handles the closing tag of an element, popping it from the appropriate stack.
    /// Returns `true` if this was the end tag for the `parse_nodes` call.
    fn handle_end_tag(&mut self, qname: QName) -> Result<bool, PipelineError> {
        let tag_name = qname.as_ref();
        debug!("Handling end tag: </{}>", String::from_utf8_lossy(tag_name));
        match tag_name {
            b"container" | b"list" | b"list-item" | b"text" | b"flex-container" => {
                if let Some(node) = self.node_stack.pop() {
                    self.push_block_to_parent(node);
                }
            }
            b"table" => { /* Popping is handled by `handle_table_start` */ }
            b"row" => {
                self.row_column_index_stack.pop();
            }
            b"cell" => {
                if let Some(idx) = self.row_column_index_stack.last_mut() {
                    *idx += 1;
                }
            }
            b"header" => self.is_in_table_header = false,
            b"tbody" | b"xsl:if" => { /* State managed by start handlers or requires no action */ }
            b"link" | b"strong" | b"b" | b"em" | b"i" => {
                if let Some(node) = self.inline_stack.pop() {
                    self.push_inline_to_parent(node);
                }
            }
            b"petty-wrapper" => return Ok(true), // End of this parsing context
            b"xsl:for-each" => { /* Already handled by capture_inner_xml */ }
            _ => {}
        }
        Ok(false)
    }
}

// Tests are now moved to the handler files, but we can keep a few high-level integration tests here.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::{InlineNode, IRNode};
    use crate::stylesheet::Color;
    use handlebars::Handlebars;
    use serde_json::json;

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
            // Check inline style
            let override_style = style_override
                .as_ref()
                .expect("Table should have an override style");
            assert_eq!(override_style.margin.as_ref().unwrap().top, 20.0);

            // Check header
            let header_node = header.as_ref().expect("Table should have a header");
            assert_eq!(header_node.rows.len(), 1, "Header should have one row");
            let header_cell = &header_node.rows[0].cells[0];
            assert_eq!(header_cell.children.len(), 1);
            assert_eq!(get_text_from_node(&header_cell.children[0]), "Header");

            // Check body
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