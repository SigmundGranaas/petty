use super::util::{
    get_attr_owned_optional, get_attr_owned_required, get_attr_required,
    parse_fo_attributes_to_element_style, parse_table_columns, OwnedAttributes,
};
use crate::error::PipelineError;
use crate::idf::{
    IRNode, InlineNode, TableBody, TableCell, TableColumnDefinition, TableHeader, TableRow,
};
use crate::xpath;
use handlebars::Handlebars;
use log::debug;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;

/// A stateful builder that constructs a single `IRNode` tree from an XML fragment.
pub(super) struct TreeBuilder<'a> {
    template_engine: &'a Handlebars<'static>,
    // A stack of nodes currently being built. The top of the stack is the current parent.
    node_stack: Vec<IRNode>,
    // A stack for inline nodes (e.g., within a paragraph).
    inline_stack: Vec<InlineNode>,
    // Tracks the current column index when parsing a table row.
    row_column_index_stack: Vec<usize>,
    // Tracks if the parser is currently inside a <table>'s <header> tag.
    is_in_table_header: bool,
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
    fn push_block_to_parent(&mut self, node: IRNode) {
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
    fn push_inline_to_parent(&mut self, node: InlineNode) {
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
    fn parse_nodes(
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
            b"xsl:value-of" => {
                let path = get_attr_required(e, b"select")?;
                let content = xpath::select_as_string(context, &path);
                debug!("  <xsl:value-of select=\"{}\"> -> \"{}\"", path, content);
                if !content.is_empty() {
                    self.push_inline_to_parent(InlineNode::Text(content));
                }
            }
            b"image" => {
                let src_template = get_attr_required(e, b"src")?;
                let attributes = e
                    .attributes()
                    .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
                    .collect::<Result<OwnedAttributes, _>>()?;

                let style_name = get_attr_owned_optional(&attributes, b"style")?;
                let style_override = parse_fo_attributes_to_element_style(&attributes)?;

                let src = self
                    .template_engine
                    .render_template(&src_template, context)
                    .map_err(|err| PipelineError::TemplateParseError(err.to_string()))?;

                let in_paragraph = matches!(self.node_stack.last(), Some(IRNode::Paragraph { .. }));

                if in_paragraph {
                    let node = InlineNode::Image {
                        src,
                        style_name,
                        style_override,
                        data: None,
                    };
                    self.push_inline_to_parent(node);
                } else {
                    let node = IRNode::Image {
                        src,
                        style_name,
                        style_override,
                        data: None,
                    };
                    self.push_block_to_parent(node);
                }
            }
            b"br" => self.push_inline_to_parent(InlineNode::LineBreak),
            // Other empty tags can be handled here
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
        let tag_name = e.name();
        debug!(
            "Handling start tag: <{}>",
            String::from_utf8_lossy(tag_name.as_ref())
        );

        let attributes = e
            .attributes()
            .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
            .collect::<Result<OwnedAttributes, _>>()?;

        match tag_name.as_ref() {
            // --- Control Flow ---
            b"xsl:for-each" => {
                let path = get_attr_owned_required(&attributes, b"select", b"xsl:for-each")?;
                debug!("Starting <xsl:for-each select=\"{}\">", path);
                let inner_xml =
                    super::super::xslt::capture_inner_xml(reader, QName(b"xsl:for-each"))?;
                let selected_values = xpath::select(context, &path);
                let items: Vec<&'a Value> = if let Some(arr) =
                    selected_values.first().and_then(|v| v.as_array())
                {
                    arr.iter().collect()
                } else {
                    selected_values
                };
                debug!("  <xsl:for-each> found {} items to iterate.", items.len());
                for (i, item_context) in items.iter().enumerate() {
                    debug!("  <xsl:for-each> processing item {}", i);
                    let mut template_reader = Reader::from_str(&inner_xml);
                    template_reader.config_mut().trim_text(false);
                    self.parse_nodes(&mut template_reader, item_context)?;
                }
                debug!("Finished <xsl:for-each select=\"{}\">", path);
            }
            b"xsl:if" => {
                let test = get_attr_owned_required(&attributes, b"test", b"xsl:if")?;
                let results = xpath::select(context, &test);
                let is_truthy = !results.is_empty()
                    && results
                    .iter()
                    .all(|v| !v.is_null() && v.as_bool() != Some(false));
                debug!("<xsl:if test=\"{}\"> evaluated to {}", test, is_truthy);
                if !is_truthy {
                    reader.read_to_end_into(tag_name, &mut vec![])?;
                }
            }

            // --- Block Elements ---
            b"container" | b"list" | b"list-item" | b"flex-container" | b"text" => {
                let style_name = get_attr_owned_optional(&attributes, b"style")?;
                let style_override = parse_fo_attributes_to_element_style(&attributes)?;

                let node = match tag_name.as_ref() {
                    b"list" => IRNode::List {
                        style_name,
                        style_override,
                        children: vec![],
                    },
                    b"list-item" => IRNode::ListItem {
                        style_name,
                        style_override,
                        children: vec![],
                    },
                    b"flex-container" => IRNode::FlexContainer {
                        style_name,
                        style_override,
                        children: vec![],
                    },
                    b"text" => IRNode::Paragraph {
                        style_name,
                        style_override,
                        children: vec![],
                    },
                    _ => IRNode::Block {
                        style_name,
                        style_override,
                        children: vec![],
                    },
                };
                self.node_stack.push(node);
            }

            // --- Inline Elements ---
            b"link" => {
                let href_template = get_attr_owned_required(&attributes, b"href", b"link")?;
                let style_name = get_attr_owned_optional(&attributes, b"style")?;
                let style_override = parse_fo_attributes_to_element_style(&attributes)?;
                let href = self
                    .template_engine
                    .render_template(&href_template, context)
                    .map_err(|err| PipelineError::TemplateParseError(err.to_string()))?;
                self.inline_stack.push(InlineNode::Hyperlink {
                    href,
                    style_name,
                    style_override,
                    children: vec![],
                });
            }
            b"strong" | b"b" => {
                let style_override = parse_fo_attributes_to_element_style(&attributes)?;
                self.inline_stack.push(InlineNode::StyledSpan {
                    style_name: Some("bold".to_string()),
                    style_override,
                    children: vec![],
                });
            }
            b"em" | b"i" => {
                let style_override = parse_fo_attributes_to_element_style(&attributes)?;
                self.inline_stack.push(InlineNode::StyledSpan {
                    style_name: Some("italic".to_string()),
                    style_override,
                    children: vec![],
                });
            }

            // --- Tables ---
            b"table" => self.handle_table_start(&attributes, reader, context)?,
            b"header" => self.handle_header_start()?,
            b"tbody" => {} // tbody is implicit, just parse children
            b"row" => self.handle_row_start()?,
            b"cell" => self.handle_cell_start(&attributes)?,

            // --- Structural / Ignored ---
            b"petty-wrapper" => {} // Descend into dummy wrapper
            b"columns" | b"column" => {
                reader.read_to_end_into(tag_name, &mut vec![])?;
            }
            _ => {
                log::warn!(
                    "Ignoring unknown start tag: {}",
                    String::from_utf8_lossy(tag_name.as_ref())
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
            b"table" => { /* This is now handled in handle_table_start */ }
            b"row" => {
                self.row_column_index_stack.pop();
            }
            b"cell" => {
                if let Some(idx) = self.row_column_index_stack.last_mut() {
                    *idx += 1;
                }
            }
            b"header" => self.is_in_table_header = false,
            b"tbody" | b"xsl:if" => { /* State managed by start handlers or requires no action */
            }
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

// --- Tag-Specific Handlers ---
impl<'a> TreeBuilder<'a> {
    fn handle_table_start(
        &mut self,
        attributes: &OwnedAttributes,
        reader: &mut Reader<&[u8]>,
        context: &'a Value,
    ) -> Result<(), PipelineError> {
        let style_name = get_attr_owned_optional(attributes, b"style")?;
        let style_override = parse_fo_attributes_to_element_style(attributes)?;

        let inner_xml = super::super::xslt::capture_inner_xml(reader, QName(b"table"))?;

        let mut columns_reader = Reader::from_str(&inner_xml);
        let mut buf = Vec::new();
        let mut col_defs = Vec::new();
        loop {
            match columns_reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"columns" => {
                    col_defs = parse_table_columns(&mut columns_reader, e.name())?
                        .into_iter()
                        .map(|c| TableColumnDefinition {
                            width: c.width,
                            style: c.style,
                            header_style: c.header_style,
                        })
                        .collect();
                    break;
                }
                Ok(XmlEvent::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => (),
            }
        }

        let table_node = IRNode::Table {
            style_name,
            style_override,
            columns: col_defs,
            calculated_widths: Vec::new(),
            header: None,
            body: Box::new(TableBody { rows: Vec::new() }),
        };
        self.node_stack.push(table_node);

        let mut content_reader = Reader::from_str(&inner_xml);
        content_reader.config_mut().trim_text(false);
        self.parse_nodes(&mut content_reader, context)?;

        if let Some(table_node) = self.node_stack.pop() {
            self.push_block_to_parent(table_node);
        }

        Ok(())
    }

    fn handle_header_start(&mut self) -> Result<(), PipelineError> {
        if let Some(IRNode::Table { header, .. }) = self.node_stack.last_mut() {
            *header = Some(Box::new(TableHeader { rows: Vec::new() }));
            self.is_in_table_header = true;
        }
        Ok(())
    }

    fn handle_row_start(&mut self) -> Result<(), PipelineError> {
        self.row_column_index_stack.push(0);
        let new_row = TableRow { cells: Vec::new() };

        match self.node_stack.last_mut() {
            Some(IRNode::Table { header, body, .. }) => {
                if self.is_in_table_header {
                    if let Some(h) = header {
                        h.rows.push(new_row);
                    }
                } else {
                    body.rows.push(new_row);
                }
            }
            _ => {
                return Err(PipelineError::TemplateParseError(
                    "<row> found outside a <table>".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn handle_cell_start(&mut self, attributes: &OwnedAttributes) -> Result<(), PipelineError> {
        let style_name = get_attr_owned_optional(attributes, b"style")?;
        let style_override = parse_fo_attributes_to_element_style(attributes)?;
        let _col_index = *self.row_column_index_stack.last().ok_or_else(|| {
            PipelineError::TemplateParseError("<cell> outside of <row>".into())
        })?;

        let new_cell = TableCell {
            style_name,
            style_override,
            children: Vec::new(),
        };

        match self.node_stack.last_mut() {
            Some(IRNode::Table { header, body, .. }) => {
                let row = if self.is_in_table_header {
                    header.as_mut().and_then(|h| h.rows.last_mut())
                } else {
                    body.rows.last_mut()
                };

                if let Some(r) = row {
                    r.cells.push(new_cell);
                } else {
                    return Err(PipelineError::TemplateParseError(
                        "<cell> found without a parent <row>".to_string(),
                    ));
                }
            }
            _ => {
                return Err(PipelineError::TemplateParseError(
                    "<cell> found outside a <table>".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_for_each_with_nested_data_and_paths() {
        let data = json!({
            "id": 123,
            "user": { "name": "Test User" },
            "items": [
                { "description": "Item A" },
                { "description": "Item B" }
            ]
        });
        let template = r#"
            <text>User: <xsl:value-of select="user/name"/></text>
            <xsl:for-each select="items">
                <text>Item: <xsl:value-of select="description"/></text>
            </xsl:for-each>
        "#;

        let tree = build_test_tree(template, &data).unwrap();

        // Expected output: 1 "User" paragraph, 2 "Item" paragraphs.
        assert_eq!(tree.len(), 3, "Should create a node for the user and each item");

        let get_text = |node: &IRNode| -> String {
            if let IRNode::Paragraph { children, .. } = node {
                // Text can be split, so we need to join it.
                return children
                    .iter()
                    .map(|inline| {
                        if let InlineNode::Text(text) = inline {
                            text.as_str()
                        } else {
                            ""
                        }
                    })
                    .collect::<String>();
            }
            panic!("Expected paragraph with text");
        };

        assert_eq!(get_text(&tree[0]), "User: Test User");
        assert_eq!(get_text(&tree[1]), "Item: Item A");
        assert_eq!(get_text(&tree[2]), "Item: Item B");
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

        // Check Paragraph 1
        let p1 = &tree[0];
        assert!(matches!(p1, IRNode::Paragraph { style_name, .. } if style_name == &Some("p1".to_string())));

        // Check Container
        let c1 = &tree[1];
        if let IRNode::Block {
            style_name,
            children,
            ..
        } = c1
        {
            assert_eq!(style_name, &Some("c1".to_string()));
            assert_eq!(children.len(), 1);
            // Check Paragraph 2 inside container
            let p2 = &children[0];
            assert!(matches!(p2, IRNode::Paragraph { style_name, .. } if style_name == &Some("p2".to_string())));
        } else {
            panic!("Expected a Block node, got {:?}", c1);
        }

        // Check Paragraph 3
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