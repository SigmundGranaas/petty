// src/parser/xslt/builder.rs
use super::util::{
    get_attr_owned_optional, get_attr_owned_required, get_owned_attributes, parse_dimension,
    parse_fo_attributes_to_element_style, OwnedAttributes,
};
use crate::idf::{IRNode, InlineNode, TableBody, TableCell, TableHeader, TableRow};
use crate::parser::ParseError;
use crate::stylesheet::{parse_attribute_set_from_event, ElementStyle, TableColumn};
use crate::xpath::{self, Condition, Selection};
use handlebars::Handlebars;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

// --- Abstract Syntax Tree (AST) for Pre-parsed Templates ---

/// Represents a pre-compiled, executable block of XSLT.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparsedTemplate(pub(super) Vec<XsltInstruction>);

/// A struct to hold pre-resolved styles for a single instruction.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct PreparsedStyles {
    /// A list of pre-resolved, shared pointers to named styles (from attribute-sets).
    pub style_sets: Vec<Arc<ElementStyle>>,
    /// An optional inline style override (from FO attributes like font-size="...").
    pub style_override: Option<ElementStyle>,
}

/// The definition of a table column, parsed at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnDefinition {
    pub width: Option<crate::stylesheet::Dimension>,
    pub style: Option<String>,
    pub header_style: Option<String>,
}

/// An instruction in a pre-parsed template, representing a node or control flow statement.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum XsltInstruction {
    /// A literal block of text, potentially with Handlebars templates.
    Text(String),
    /// A standard content tag like `<container>` or `<text>`.
    ContentTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        body: PreparsedTemplate,
    },
    /// A self-closing tag like `<br/>` or `<image>`.
    EmptyTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
    },
    /// A fully structured `if` block with a compiled condition.
    If {
        test: Condition,
        body: PreparsedTemplate,
    },
    /// A fully structured `for-each` block with a compiled selection path.
    ForEach {
        select: Selection,
        body: PreparsedTemplate,
    },
    /// A `value-of` instruction with a compiled selection path.
    ValueOf {
        select: Selection,
    },
    /// A `call-template` instruction with a compiled name.
    CallTemplate {
        name: String,
    },
    /// A structured `table` block, with pre-parsed components.
    Table {
        styles: PreparsedStyles,
        columns: Vec<TableColumnDefinition>,
        header: Option<PreparsedTemplate>,
        body: PreparsedTemplate,
    },
}

// --- Phase 1: The Compiler ---

/// A stateful compiler that transforms an XSLT string into a `PreparsedTemplate` AST.
pub(crate) struct Compiler;

impl Compiler {
    /// The main entry point for compiling an XSLT string. It performs a single pass,
    /// extracting styles, the root template (`match="/"`) and all named templates.
    pub fn compile(
        full_xslt_str: &str,
    ) -> Result<
        (
            PreparsedTemplate,                 // Root template body
            HashMap<String, PreparsedTemplate>, // Named templates
            HashMap<String, Arc<ElementStyle>>, // Styles
        ),
        ParseError,
    > {
        let styles = Self::extract_styles(full_xslt_str)?;
        let mut reader = Reader::from_str(full_xslt_str);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut root_template = None;
        let mut named_templates = HashMap::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:template" => {
                    let attributes = get_owned_attributes(&e)?;
                    if let Some(match_attr) = get_attr_owned_optional(&attributes, b"match")? {
                        if match_attr == "/" {
                            root_template = Some(Self::compile_body(&mut reader, e.name(), &styles)?);
                        } else {
                            // Skip other templates with `match` attributes for now
                            reader.read_to_end_into(e.name(), &mut Vec::new())?;
                        }
                    } else if let Some(name) = get_attr_owned_optional(&attributes, b"name")? {
                        let body = Self::compile_body(&mut reader, e.name(), &styles)?;
                        named_templates.insert(name, body);
                    }
                }
                XmlEvent::Eof => break,
                _ => (),
            }
            buf.clear();
        }

        let root = root_template.ok_or_else(|| {
            ParseError::TemplateParse(
                "Could not find root <xsl:template match=\"/\">".to_string(),
            )
        })?;

        Ok((root, named_templates, styles))
    }

    /// PASS 1: Populates the compiler's internal style map.
    fn extract_styles(xml_str: &str) -> Result<HashMap<String, Arc<ElementStyle>>, ParseError> {
        let mut reader = Reader::from_str(xml_str);
        let mut buf = Vec::new();
        let mut style_buf = Vec::new();
        let mut styles = HashMap::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:attribute-set" => {
                    let (name, style) =
                        parse_attribute_set_from_event(&mut reader, &e, &mut style_buf)?;
                    styles.insert(name, Arc::new(style));
                }
                XmlEvent::Eof => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(styles)
    }

    /// Recursively compiles the body of an XML tag until its corresponding end tag is found.
    fn compile_body(
        reader: &mut Reader<&[u8]>,
        end_qname: QName,
        styles: &HashMap<String, Arc<ElementStyle>>,
    ) -> Result<PreparsedTemplate, ParseError> {
        let mut instructions = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => {
                    instructions.push(Self::compile_start_tag(reader, e.name(), get_owned_attributes(&e)?, styles)?);
                }
                XmlEvent::Empty(e) => {
                    instructions.push(Self::compile_empty_tag(&e, styles)?);
                }
                XmlEvent::Text(e) => {
                    let text = e.unescape()?.into_owned();
                    if !text.trim().is_empty() {
                        instructions.push(XsltInstruction::Text(text));
                    }
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                XmlEvent::Eof => {
                    if end_qname.as_ref() != b"petty-wrapper" {
                        return Err(ParseError::TemplateParse(format!(
                            "Unexpected EOF while compiling template. Expected end tag </{}>.",
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

    /// Compiles a start tag, resolving styles and parsing expressions immediately.
    fn compile_start_tag(
        reader: &mut Reader<&[u8]>,
        tag_name: QName,
        attributes: OwnedAttributes,
        styles: &HashMap<String, Arc<ElementStyle>>,
    ) -> Result<XsltInstruction, ParseError> {
        match tag_name.as_ref() {
            b"xsl:for-each" => {
                let select_xpath = get_attr_owned_required(&attributes, b"select", b"xsl:for-each")?;
                let select = xpath::parse_selection(&select_xpath)?;
                let body = Self::compile_body(reader, tag_name, styles)?;
                Ok(XsltInstruction::ForEach { select, body })
            }
            b"xsl:if" => {
                let test_xpath = get_attr_owned_required(&attributes, b"test", b"xsl:if")?;
                let test = xpath::parse_condition(&test_xpath)?;
                let body = Self::compile_body(reader, tag_name, styles)?;
                Ok(XsltInstruction::If { test, body })
            }
            b"fo:table" | b"table" => {
                let preparsed_styles = Self::resolve_styles(&attributes, styles)?;
                Self::compile_table(reader, tag_name, preparsed_styles, styles)
            }
            _ => {
                let preparsed_styles = Self::resolve_styles(&attributes, styles)?;
                let body = Self::compile_body(reader, tag_name, styles)?;
                Ok(XsltInstruction::ContentTag {
                    tag_name: tag_name.as_ref().to_vec(),
                    styles: preparsed_styles,
                    body,
                })
            }
        }
    }

    /// Compiles an empty tag into the appropriate `XsltInstruction`.
    fn compile_empty_tag(e: &BytesStart, styles: &HashMap<String, Arc<ElementStyle>>) -> Result<XsltInstruction, ParseError> {
        let attributes = get_owned_attributes(e)?;
        match e.name().as_ref() {
            b"xsl:value-of" => {
                let select_xpath = get_attr_owned_required(&attributes, b"select", b"xsl:value-of")?;
                let select = xpath::parse_selection(&select_xpath)?;
                Ok(XsltInstruction::ValueOf { select })
            }
            b"xsl:call-template" => {
                let name = get_attr_owned_required(&attributes, b"name", b"xsl:call-template")?;
                Ok(XsltInstruction::CallTemplate { name })
            }
            _ => {
                let preparsed_styles = Self::resolve_styles(&attributes, styles)?;
                Ok(XsltInstruction::EmptyTag {
                    tag_name: e.name().as_ref().to_vec(),
                    styles: preparsed_styles,
                })
            }
        }
    }

    /// Helper function to resolve styles for a tag at compile time.
    fn resolve_styles(attributes: &OwnedAttributes, styles: &HashMap<String, Arc<ElementStyle>>) -> Result<PreparsedStyles, ParseError> {
        let mut style_sets = Vec::new();

        if let Some(names_str) = get_attr_owned_optional(attributes, b"style")? {
            for name in names_str.split_whitespace() {
                let style_arc = styles.get(name).ok_or_else(|| {
                    ParseError::TemplateParse(format!(
                        "Style '{}' not found. It must be defined in an <xsl:attribute-set>.",
                        name
                    ))
                })?;
                style_sets.push(Arc::clone(style_arc));
            }
        }

        if let Some(names_str) = get_attr_owned_optional(attributes, b"use-attribute-sets")? {
            for name in names_str.split_whitespace() {
                let style_arc = styles.get(name).ok_or_else(|| {
                    ParseError::TemplateParse(format!("Attribute set '{}' not found.", name))
                })?;
                style_sets.push(Arc::clone(style_arc));
            }
        }

        let style_override = parse_fo_attributes_to_element_style(attributes)?;

        Ok(PreparsedStyles {
            style_sets,
            style_override,
        })
    }

    /// Special-cased compiler for `<table>`.
    fn compile_table(
        reader: &mut Reader<&[u8]>,
        end_qname: QName,
        styles: PreparsedStyles,
        style_map: &HashMap<String, Arc<ElementStyle>>,
    ) -> Result<XsltInstruction, ParseError> {
        let mut columns = Vec::new();
        let mut header: Option<PreparsedTemplate> = None;
        let mut body_instructions = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => {
                    let tag_name = e.name();
                    match tag_name.as_ref() {
                        b"columns" => columns = Self::compile_columns(reader, tag_name)?,
                        b"header" | b"fo:table-header" => header = Some(Self::compile_body(reader, tag_name, style_map)?),
                        b"tbody" | b"fo:table-body" => {
                            let template = Self::compile_body(reader, tag_name, style_map)?;
                            body_instructions.extend(template.0);
                        }
                        _ => {
                            let instruction = Self::compile_start_tag(reader, tag_name, get_owned_attributes(&e)?, style_map)?;
                            body_instructions.push(instruction);
                        }
                    }
                }
                XmlEvent::Empty(e) => {
                    let instruction = Self::compile_empty_tag(&e, style_map)?;
                    body_instructions.push(instruction);
                }
                XmlEvent::Text(e) => {
                    let text = e.unescape()?.into_owned();
                    if !text.trim().is_empty() {
                        body_instructions.push(XsltInstruction::Text(text));
                    }
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                XmlEvent::Eof => return Err(ParseError::TemplateParse(format!(
                    "Unexpected EOF while parsing table. Expected end tag </{}>.",
                    String::from_utf8_lossy(end_qname.as_ref())
                ))),
                _ => {}
            }
            buf.clear();
        }

        Ok(XsltInstruction::Table {
            styles,
            columns,
            header,
            body: PreparsedTemplate(body_instructions),
        })
    }

    /// Compiles the contents of a `<columns>` tag.
    fn compile_columns(
        reader: &mut Reader<&[u8]>,
        end_qname: QName,
    ) -> Result<Vec<TableColumnDefinition>, ParseError> {
        let mut columns = Vec::new();
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Empty(e) if e.name().as_ref() == b"column" || e.name().as_ref() == b"fo:table-column" => {
                    let mut col = TableColumn::default();
                    for attr in e.attributes().flatten() {
                        let value = attr.decode_and_unescape_value(reader.decoder())?;
                        match attr.key.as_ref() {
                            b"column-width" | b"width" => col.width = parse_dimension(&value),
                            b"header-style" => col.header_style = Some(value.into_owned()),
                            b"style" => col.style = Some(value.into_owned()),
                            _ => {}
                        }
                    }
                    columns.push(TableColumnDefinition {
                        width: col.width,
                        style: col.style,
                        header_style: col.header_style,
                    });
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                _ => {}
            }
        }
        Ok(columns)
    }
}

/// A stateful builder that constructs an `IRNode` tree by executing a `PreparsedTemplate`.
pub struct TreeBuilder<'h> {
    template_engine: &'h Handlebars<'static>,
    named_templates: &'h HashMap<String, PreparsedTemplate>,
    node_stack: Vec<IRNode>,
    inline_stack: Vec<InlineNode>,
    is_in_table_header: bool,
}

impl<'h> TreeBuilder<'h> {
    pub fn new(
        template_engine: &'h Handlebars<'static>,
        named_templates: &'h HashMap<String, PreparsedTemplate>,
    ) -> Self {
        Self {
            template_engine,
            named_templates,
            node_stack: vec![],
            inline_stack: vec![],
            is_in_table_header: false,
        }
    }

    /// The main public entry point for the executor.
    pub fn build_tree_from_preparsed(
        &mut self,
        template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<Vec<IRNode>, ParseError> {
        self.node_stack.clear();
        self.inline_stack.clear();
        self.is_in_table_header = false;

        let root_node = IRNode::Root(Vec::with_capacity(16));
        self.node_stack.push(root_node);

        self.execute_template(template, context)?;

        if let Some(IRNode::Root(children)) = self.node_stack.pop() {
            Ok(children)
        } else {
            Err(ParseError::TemplateParse(
                "Failed to construct root node.".to_string(),
            ))
        }
    }

    /// Recursively walks the AST and builds the `IRNode` tree.
    fn execute_template(
        &mut self,
        template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<(), ParseError> {
        for instruction in &template.0 {
            match instruction {
                XsltInstruction::ForEach { select, body } => {
                    let selected_values = select.select(context);
                    let items: Vec<&Value> = if let Some(arr) = selected_values.first().and_then(|v| v.as_array()) {
                        arr.iter().collect()
                    } else {
                        selected_values
                    };
                    for item_context in items {
                        self.execute_template(body, item_context)?;
                    }
                }
                XsltInstruction::If { test, body } => {
                    if test.evaluate(context) {
                        self.execute_template(body, context)?;
                    }
                }
                XsltInstruction::ContentTag { tag_name, styles, body } => {
                    self.execute_start_tag(tag_name, styles, context)?;
                    self.execute_template(body, context)?;
                    self.execute_end_tag(tag_name)?;
                }
                XsltInstruction::EmptyTag { tag_name, styles } => {
                    self.execute_empty_tag(tag_name, styles, context)?;
                }
                XsltInstruction::Text(text) => {
                    let rendered_text = if text.contains("{{") {
                        self.render_text(text, context)?
                    } else {
                        text.clone()
                    };
                    self.push_inline_to_parent(InlineNode::Text(rendered_text));
                }
                XsltInstruction::ValueOf { select } => {
                    let content = xpath::select_as_string(select, context);
                    if !content.is_empty() {
                        self.push_inline_to_parent(InlineNode::Text(content));
                    }
                }
                XsltInstruction::CallTemplate { name } => {
                    let target_template = self.named_templates.get(name).ok_or_else(|| {
                        ParseError::TemplateParse(format!(
                            "Called template '{}' not found in stylesheet.",
                            name
                        ))
                    })?;
                    // Recursive call with the same context
                    self.execute_template(target_template, context)?;
                }
                XsltInstruction::Table { styles, columns, header, body } => {
                    self.execute_table(styles, columns, header.as_ref(), body, context)?;
                }
            }
        }
        Ok(())
    }

    fn execute_start_tag(
        &mut self,
        tag_name: &[u8],
        styles: &PreparsedStyles,
        context: &Value,
    ) -> Result<(), ParseError> {
        let style_sets = styles.style_sets.clone();
        let style_override = styles.style_override.clone();

        match String::from_utf8_lossy(tag_name).as_ref() {
            "fo:list-block" | "list" => self.node_stack.push(IRNode::List { style_sets, style_override, children: vec![] }),
            "fo:list-item" | "list-item" => self.node_stack.push(IRNode::ListItem { style_sets, style_override, children: vec![] }),
            "flex-container" => self.node_stack.push(IRNode::FlexContainer { style_sets, style_override, children: vec![] }),
            "fo:block" | "text" => self.node_stack.push(IRNode::Paragraph { style_sets, style_override, children: vec![] }),
            "fo:basic-link" | "link" => {
                // Non-style attributes like `href` would need a more specific instruction type.
                // For now, we render it from the context if possible or default to empty.
                let href_template = ""; // Placeholder. A more robust solution would parse this during compilation.
                let href = self.render_text(href_template, context)?;
                self.inline_stack.push(InlineNode::Hyperlink { href, style_sets, style_override, children: vec![] });
            }
            "fo:inline" | "strong" | "b" | "em" | "i" => {
                // A more advanced version could merge a 'bold'/'italic' style set here.
                self.inline_stack.push(InlineNode::StyledSpan { style_sets, style_override, children: vec![] });
            }
            "fo:table-row" | "row" => {
                let new_row = TableRow { cells: Vec::with_capacity(8) };
                if let Some(IRNode::Table { header, body, .. }) = self.node_stack.last_mut() {
                    if self.is_in_table_header {
                        if let Some(h) = header { h.rows.push(new_row); }
                    } else {
                        body.rows.push(new_row);
                    }
                }
            }
            "fo:table-cell" | "cell" => {
                let new_cell = TableCell { style_sets, style_override, children: Vec::with_capacity(2) };
                if let Some(IRNode::Table { header, body, .. }) = self.node_stack.last_mut() {
                    let row = if self.is_in_table_header {
                        header.as_mut().and_then(|h| h.rows.last_mut())
                    } else {
                        body.rows.last_mut()
                    };
                    if let Some(r) = row { r.cells.push(new_cell); }
                }
            }
            // Default to a generic block container.
            _ => self.node_stack.push(IRNode::Block { style_sets, style_override, children: vec![] }),
        }
        Ok(())
    }

    fn execute_end_tag(&mut self, tag_name: &[u8]) -> Result<(), ParseError> {
        match tag_name {
            b"fo:basic-link" | b"link" | b"fo:inline" | b"strong" | b"b" | b"em" | b"i" => {
                if let Some(node) = self.inline_stack.pop() { self.push_inline_to_parent(node); }
            }
            b"fo:table-row" | b"row" | b"fo:table-cell" | b"cell" => { /* No op */ }
            _ => { // Assume it's a block-level tag
                if let Some(node) = self.node_stack.pop() { self.push_block_to_parent(node); }
            }
        }
        Ok(())
    }

    fn execute_empty_tag(
        &mut self,
        tag_name: &[u8],
        styles: &PreparsedStyles,
        _context: &Value,
    ) -> Result<(), ParseError> {
        match tag_name {
            b"fo:external-graphic" | b"image" => {
                // A more robust solution would parse the 'src' attribute during compilation.
                let src = "".to_string();
                let style_sets = styles.style_sets.clone();
                let style_override = styles.style_override.clone();

                if matches!(self.node_stack.last(), Some(IRNode::Paragraph { .. })) {
                    self.push_inline_to_parent(InlineNode::Image { src, style_sets, style_override, data: None });
                } else {
                    self.push_block_to_parent(IRNode::Image { src, style_sets, style_override, data: None });
                }
            }
            b"fo:block" | b"br" => self.push_inline_to_parent(InlineNode::LineBreak),
            _ => {}
        }
        Ok(())
    }

    fn execute_table(
        &mut self,
        styles: &PreparsedStyles,
        columns: &[TableColumnDefinition],
        header_template: Option<&PreparsedTemplate>,
        body_template: &PreparsedTemplate,
        context: &Value,
    ) -> Result<(), ParseError> {
        let table_node = IRNode::Table {
            style_sets: styles.style_sets.clone(),
            style_override: styles.style_override.clone(),
            columns: columns.iter().map(|c| crate::idf::TableColumnDefinition {
                width: c.width.clone(), style: c.style.clone(), header_style: c.header_style.clone()
            }).collect(),
            calculated_widths: Vec::new(),
            header: if header_template.is_some() { Some(Box::new(TableHeader { rows: vec![] })) } else { None },
            body: Box::new(TableBody { rows: vec![] }),
        };
        self.node_stack.push(table_node);

        if let Some(template) = header_template {
            self.is_in_table_header = true;
            self.execute_template(template, context)?;
            self.is_in_table_header = false;
        }

        self.execute_template(body_template, context)?;

        if let Some(node) = self.node_stack.pop() {
            self.push_block_to_parent(node);
        }
        Ok(())
    }

    fn render_text(&self, text: &str, context: &Value) -> Result<String, ParseError> {
        self.template_engine.render_template(text, context).map_err(|e| ParseError::TemplateRender(e.to_string()))
    }

    fn push_block_to_parent(&mut self, node: IRNode) {
        match self.node_stack.last_mut() {
            Some(IRNode::Root(children))
            | Some(IRNode::Block { children, .. })
            | Some(IRNode::FlexContainer { children, .. })
            | Some(IRNode::List { children, .. })
            | Some(IRNode::ListItem { children, .. }) => children.push(node),
            Some(IRNode::Table { .. }) => {
                let row = if self.is_in_table_header {
                    if let Some(IRNode::Table { header: Some(h), .. }) = self.node_stack.last_mut() { h.rows.last_mut() } else { None }
                } else {
                    if let Some(IRNode::Table { body, .. }) = self.node_stack.last_mut() { body.rows.last_mut() } else { None }
                };
                if let Some(r) = row {
                    if let Some(cell) = r.cells.last_mut() { cell.children.push(node); }
                }
            },
            _ => log::warn!("Cannot add block node to current parent."),
        }
    }

    fn push_inline_to_parent(&mut self, node: InlineNode) {
        match self.inline_stack.last_mut() {
            Some(InlineNode::StyledSpan { children, .. }) | Some(InlineNode::Hyperlink { children, .. }) => children.push(node),
            _ => {
                if let Some(IRNode::Paragraph { children, .. }) = self.node_stack.last_mut() {
                    children.push(node);
                } else if let Some(IRNode::Table { .. }) = self.node_stack.last_mut() {
                    self.node_stack.push(IRNode::Paragraph { style_sets: vec![], style_override: None, children: vec![node] });
                    let p_node = self.node_stack.pop().unwrap();
                    self.push_block_to_parent(p_node);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::idf::IRNode::Paragraph;

    #[test]
    fn test_compile_named_and_root_templates() {
        let xslt = r#"
            <xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
                <xsl:template name="named">
                    <text>Named Template</text>
                </xsl:template>
                <xsl:template match="/">
                    <text>Root Template</text>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let (root, named, _) = Compiler::compile(xslt).unwrap();
        assert_eq!(named.len(), 1);
        assert!(named.contains_key("named"));

        let named_template = &named["named"];
        assert_eq!(named_template.0.len(), 1);
        match &named_template.0[0] {
            XsltInstruction::ContentTag { tag_name, .. } => assert_eq!(tag_name.as_slice(), b"text"),
            _ => panic!("Expected ContentTag"),
        }

        assert_eq!(root.0.len(), 1);
        match &root.0[0] {
            XsltInstruction::ContentTag { tag_name, .. } => assert_eq!(tag_name.as_slice(), b"text"),
            _ => panic!("Expected ContentTag"),
        }
    }

    #[test]
    fn test_compile_call_template() {
        let xslt = r#"<xsl:call-template name="my-template"/>"#;
        let wrapped_xslt = format!("<wrapper>{}</wrapper>", xslt);
        let mut reader = Reader::from_str(&wrapped_xslt);
        let mut buf = vec![];
        reader.read_event_into(&mut buf).unwrap(); // consume wrapper
        buf.clear();
        let event = reader.read_event_into(&mut buf).unwrap();

        let instruction = match event {
            XmlEvent::Empty(e) => Compiler::compile_empty_tag(&e, &HashMap::new()).unwrap(),
            _ => panic!("Expected Empty tag"),
        };

        assert_eq!(
            instruction,
            XsltInstruction::CallTemplate {
                name: "my-template".to_string()
            }
        );
    }

    #[test]
    fn test_tree_builder_call_template() {
        let named_template_body = PreparsedTemplate(vec![
            XsltInstruction::ContentTag {
                tag_name: b"text".to_vec(),
                styles: Default::default(),
                body: PreparsedTemplate(vec![XsltInstruction::Text("from named".to_string())])
            }
        ]);
        let mut named_templates = HashMap::new();
        named_templates.insert("my-template".to_string(), named_template_body);

        let main_template = PreparsedTemplate(vec![
            XsltInstruction::ContentTag {
                tag_name: b"text".to_vec(),
                styles: Default::default(),
                body: PreparsedTemplate(vec![XsltInstruction::Text("from main".to_string())])
            },
            XsltInstruction::CallTemplate { name: "my-template".to_string() }
        ]);

        let handlebars = Handlebars::new();
        let mut builder = TreeBuilder::new(&handlebars, &named_templates);

        let result = builder.build_tree_from_preparsed(&main_template, &json!({})).unwrap();

        assert_eq!(result.len(), 2);

        // Check first node from main template
        if let Paragraph { children, .. } = &result[0] {
            assert_eq!(children.len(), 1);
            if let InlineNode::Text(s) = &children[0] {
                assert_eq!(s, "from main");
            } else {
                panic!("Expected InlineNode::Text");
            }
        } else {
            panic!("Expected IRNode::Paragraph for first element");
        }

        // Check second node from called template
        if let Paragraph { children, .. } = &result[1] {
            assert_eq!(children.len(), 1);
            if let InlineNode::Text(s) = &children[0] {
                assert_eq!(s, "from named");
            } else {
                panic!("Expected InlineNode::Text");
            }
        } else {
            panic!("Expected IRNode::Paragraph for second element");
        }
    }
}