use super::ast::{
    CompiledStylesheet, PreparsedStyles, PreparsedTemplate, TableColumnDefinition, TemplateRule,
    WithParam, XsltInstruction,
};
use super::util::{
    get_attr_owned_optional, get_attr_owned_required, get_line_col_from_pos, get_owned_attributes,
    OwnedAttributes,
};
use crate::parser::style::{self, apply_style_property};
use crate::parser::{Location, ParseError};
use crate::xpath;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use crate::core::style::stylesheet::{ElementStyle, TableColumn};

/// A stateful compiler that transforms an XSLT string into a `PreparsedTemplate` AST.
pub struct Compiler;

/// Returns a set of all tag names that the engine has specific logic for.
fn get_supported_tags() -> HashSet<&'static str> {
    [
        // XSLT control flow & data
        "xsl:for-each",
        "xsl:if",
        "xsl:value-of",
        "xsl:call-template",
        "xsl:apply-templates",
        "xsl:with-param",
        "xsl:param",
        // Block elements
        "page-sequence",
        "block",
        "fo:block",
        "p",
        "list",
        "fo:list-block",
        "list-item",
        "fo:list-item",
        "flex-container",
        "table",
        "fo:table",
        "header",
        "fo:table-header",
        "tbody",
        "fo:table-body",
        "row",
        "fo:table-row",
        "cell",
        "fo:table-cell",
        "columns",
        "column",
        "fo:table-column",
        // Inline elements
        "text",
        "span",
        "fo:inline",
        "strong",
        "b",
        "em",
        "i",
        "link",
        "fo:basic-link",
        "image",
        "fo:external-graphic",
        "br",
    ]
        .into()
}

impl Compiler {
    /// The main entry point for compiling an XSLT string. It performs a single pass,
    /// extracting styles and all templates into a structured `CompiledStylesheet`.
    pub fn compile(full_xslt_str: &str) -> Result<CompiledStylesheet, ParseError> {
        let styles = Self::extract_styles(full_xslt_str)?;
        let mut reader = Reader::from_str(full_xslt_str);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut stylesheet = CompiledStylesheet {
            styles,
            ..Default::default()
        };

        loop {
            let pos = reader.buffer_position() as usize;
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:template" => {
                    let attributes = get_owned_attributes(&e)?;
                    if let Some(match_attr) = get_attr_owned_optional(&attributes, b"match")? {
                        // This is a match-based template rule.
                        let body = Self::compile_body(
                            &mut reader,
                            e.name(),
                            &stylesheet.styles,
                            full_xslt_str,
                        )?;
                        if match_attr == "/" {
                            stylesheet.root_template = Some(body);
                        } else {
                            let mode = get_attr_owned_optional(&attributes, b"mode")?;
                            let priority_str = get_attr_owned_optional(&attributes, b"priority")?;
                            let priority = if let Some(p_str) = priority_str {
                                p_str.parse::<f64>().map_err(|_| ParseError::TemplateSyntax {
                                    msg: format!("Invalid priority value: '{}'", p_str),
                                    location: get_line_col_from_pos(full_xslt_str, pos).into(),
                                })?
                            } else {
                                Self::calculate_default_priority(&match_attr)
                            };

                            let rule = TemplateRule {
                                match_pattern: match_attr,
                                priority,
                                mode: mode.clone(),
                                body,
                            };
                            stylesheet.template_rules.entry(mode).or_default().push(rule);
                        }
                    } else if let Some(name) = get_attr_owned_optional(&attributes, b"name")? {
                        // This is a named template.
                        let body = Self::compile_body(
                            &mut reader,
                            e.name(),
                            &stylesheet.styles,
                            full_xslt_str,
                        )?;
                        stylesheet.named_templates.insert(name, body);
                    }
                }
                XmlEvent::Eof => break,
                _ => (),
            }
            buf.clear();
        }

        if stylesheet.root_template.is_none() {
            return Err(ParseError::TemplateParse(
                "Could not find root <xsl:template match=\"/\">. This is required.".to_string(),
            ));
        }

        // Sort all rule sets by priority (highest first)
        for rules in stylesheet.template_rules.values_mut() {
            rules.sort_by(|a, b| {
                b.priority
                    .partial_cmp(&a.priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(stylesheet)
    }

    /// Calculates the default priority of a match pattern based on its specificity.
    fn calculate_default_priority(pattern: &str) -> f64 {
        match pattern {
            "*" => -0.5,
            p if p.contains(':') || p.contains('/') => 0.0, // More complex paths
            "text()" | "comment()" | "processing-instruction()" => -0.25,
            _ => 0.0, // Specific name test
        }
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
                        Self::parse_attribute_set(&mut reader, &e, &mut style_buf)?;
                    styles.insert(name, Arc::new(style));
                }
                XmlEvent::Eof => break,
                _ => (),
            }
            buf.clear();
        }
        Ok(styles)
    }

    /// Parses the contents of an `<xsl:attribute-set>` tag.
    fn parse_attribute_set(
        reader: &mut Reader<&[u8]>,
        start_event: &BytesStart,
        buf: &mut Vec<u8>,
    ) -> Result<(String, ElementStyle), ParseError> {
        let attrs = get_owned_attributes(start_event)?;
        let name = get_attr_owned_optional(&attrs, b"name")?
            .ok_or_else(|| ParseError::TemplateParse("xsl:attribute-set missing name".into()))?;
        let mut style = ElementStyle::default();

        loop {
            match reader.read_event_into(buf) {
                Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"xsl:attribute" => {
                    let attr_attrs = get_owned_attributes(&e)?;
                    let attr_name =
                        get_attr_owned_optional(&attr_attrs, b"name")?.ok_or_else(|| {
                            ParseError::TemplateParse("xsl:attribute missing name".into())
                        })?;

                    if attr_name == "use-attribute-sets" {
                        reader.read_to_end_into(e.name(), &mut Vec::new())?;
                        continue;
                    }

                    let mut value = String::new();
                    let mut content_buf = Vec::<u8>::new();
                    let end_tag = e.name().to_owned();

                    loop {
                        match reader.read_event_into(&mut content_buf) {
                            Ok(XmlEvent::Text(text)) => value = text.unescape()?.to_string(),
                            Ok(XmlEvent::End(end)) if end.name() == end_tag => break,
                            Ok(XmlEvent::Eof) => {
                                return Err(ParseError::TemplateParse(
                                    "Unexpected EOF in xsl:attribute".into(),
                                ))
                            }
                            Err(e) => return Err(e.into()),
                            _ => {}
                        }
                        content_buf.clear();
                    }
                    if !value.trim().is_empty() {
                        apply_style_property(&mut style, &attr_name, &value)?;
                    }
                }
                Ok(XmlEvent::End(e)) if e.name().as_ref() == b"xsl:attribute-set" => break,
                Ok(XmlEvent::Eof) => {
                    return Err(ParseError::TemplateParse(
                        "Unexpected EOF in xsl:attribute-set".into(),
                    ))
                }
                Err(e) => return Err(e.into()),
                _ => (),
            }
            buf.clear();
        }
        Ok((name, style))
    }

    /// Recursively compiles the body of an XML tag until its corresponding end tag is found.
    fn compile_body(
        reader: &mut Reader<&[u8]>,
        end_qname: QName,
        styles: &HashMap<String, Arc<ElementStyle>>,
        full_xslt_str: &str,
    ) -> Result<PreparsedTemplate, ParseError> {
        let mut instructions = Vec::new();
        let mut buf = Vec::new();

        loop {
            let pos = reader.buffer_position() as usize;
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => {
                    instructions.push(Self::compile_start_tag(
                        reader,
                        e.name(),
                        get_owned_attributes(&e)?,
                        styles,
                        pos,
                        full_xslt_str,
                    )?);
                }
                XmlEvent::Empty(e) => {
                    instructions.push(Self::compile_empty_tag(&e, styles, pos, full_xslt_str)?);
                }
                XmlEvent::Text(e) => {
                    let text = e.unescape()?.into_owned();
                    if !text.trim().is_empty() {
                        instructions.push(XsltInstruction::Text(text));
                    }
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                XmlEvent::Eof => {
                    let (line, col) = get_line_col_from_pos(full_xslt_str, pos);
                    return Err(ParseError::TemplateSyntax {
                        msg: format!(
                            "Unexpected EOF while compiling template. Expected end tag for </{}>.",
                            String::from_utf8_lossy(end_qname.as_ref())
                        ),
                        location: Location { line, col },
                    });
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
        pos: usize,
        full_xslt_str: &str,
    ) -> Result<XsltInstruction, ParseError> {
        let q_name = tag_name;
        let tag_name_str = String::from_utf8_lossy(q_name.as_ref());
        if !get_supported_tags().contains(tag_name_str.as_ref()) {
            let (line, col) = get_line_col_from_pos(full_xslt_str, pos);
            log::warn!(
                "Unsupported tag <{}> found at line {}, column {}. It will be treated as a generic block container, which may not produce the desired layout.",
                tag_name_str, line, col
            );
        }

        match q_name.as_ref() {
            b"xsl:for-each" => {
                let select_xpath = get_attr_owned_required(
                    &attributes,
                    b"select",
                    b"xsl:for-each",
                    pos,
                    full_xslt_str,
                )?;
                let select = xpath::parse_selection(&select_xpath)?;
                let body = Self::compile_body(reader, q_name, styles, full_xslt_str)?;
                Ok(XsltInstruction::ForEach { select, body })
            }
            b"xsl:if" => {
                let test_xpath =
                    get_attr_owned_required(&attributes, b"test", b"xsl:if", pos, full_xslt_str)?;
                let test = xpath::parse_condition(&test_xpath)?;
                let body = Self::compile_body(reader, q_name, styles, full_xslt_str)?;
                Ok(XsltInstruction::If { test, body })
            }
            b"xsl:call-template" => {
                let name = get_attr_owned_required(
                    &attributes,
                    b"name",
                    b"xsl:call-template",
                    pos,
                    full_xslt_str,
                )?;
                let params = Self::compile_with_params(reader, q_name, full_xslt_str)?;
                Ok(XsltInstruction::CallTemplate { name, params })
            }
            b"xsl:param" => {
                reader.read_to_end_into(q_name, &mut Vec::new())?;
                Ok(XsltInstruction::Text("".into()))
            }
            b"fo:table" | b"table" => {
                let preparsed_styles =
                    Self::resolve_styles(&attributes, styles, pos, full_xslt_str)?;
                Self::compile_table(reader, q_name, preparsed_styles, styles, full_xslt_str)
            }
            _ => {
                let preparsed_styles =
                    Self::resolve_styles(&attributes, styles, pos, full_xslt_str)?;
                let attrs = Self::get_non_style_attributes(&attributes)?;
                let body = Self::compile_body(reader, q_name, styles, full_xslt_str)?;
                Ok(XsltInstruction::ContentTag {
                    tag_name: q_name.as_ref().to_vec(),
                    styles: preparsed_styles,
                    attrs,
                    body,
                })
            }
        }
    }

    /// Compiles an empty tag into the appropriate `XsltInstruction`.
    fn compile_empty_tag(
        e: &BytesStart,
        styles: &HashMap<String, Arc<ElementStyle>>,
        pos: usize,
        full_xslt_str: &str,
    ) -> Result<XsltInstruction, ParseError> {
        let attributes = get_owned_attributes(e)?;
        let q_name = e.name();
        let tag_name_str = String::from_utf8_lossy(q_name.as_ref());

        if !get_supported_tags().contains(tag_name_str.as_ref()) {
            let (line, col) = get_line_col_from_pos(full_xslt_str, pos);
            log::warn!(
                "Unsupported empty tag <{}> found at line {}, column {}. This tag will be ignored.",
                tag_name_str,
                line,
                col
            );
        }

        match q_name.as_ref() {
            b"xsl:value-of" => {
                let select_xpath = get_attr_owned_required(
                    &attributes,
                    b"select",
                    b"xsl:value-of",
                    pos,
                    full_xslt_str,
                )?;
                let select = xpath::parse_selection(&select_xpath)?;
                Ok(XsltInstruction::ValueOf { select })
            }
            b"xsl:apply-templates" => {
                let select_str = get_attr_owned_optional(&attributes, b"select")?;
                let select = select_str.map(|s| xpath::parse_selection(&s)).transpose()?;
                let mode = get_attr_owned_optional(&attributes, b"mode")?;
                Ok(XsltInstruction::ApplyTemplates { select, mode })
            }
            b"xsl:with-param" | b"xsl:param" => Ok(XsltInstruction::Text("".into())),
            _ => {
                let preparsed_styles =
                    Self::resolve_styles(&attributes, styles, pos, full_xslt_str)?;
                let attrs = Self::get_non_style_attributes(&attributes)?;
                Ok(XsltInstruction::EmptyTag {
                    tag_name: q_name.as_ref().to_vec(),
                    styles: preparsed_styles,
                    attrs,
                })
            }
        }
    }

    /// Compiles the children of a `<xsl:call-template>` tag.
    fn compile_with_params(
        reader: &mut Reader<&[u8]>,
        end_qname: QName,
        full_xslt_str: &str,
    ) -> Result<Vec<WithParam>, ParseError> {
        let mut params = Vec::new();
        let mut buf = Vec::new();
        loop {
            let pos = reader.buffer_position() as usize;
            let event = reader.read_event_into(&mut buf)?;

            match event {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:with-param" => {
                    let attrs = get_owned_attributes(&e)?;
                    let name = get_attr_owned_required(
                        &attrs,
                        b"name",
                        b"xsl:with-param",
                        pos,
                        full_xslt_str,
                    )?;
                    let select_str = get_attr_owned_required(
                        &attrs,
                        b"select",
                        b"xsl:with-param",
                        pos,
                        full_xslt_str,
                    )?;
                    let select = xpath::parse_selection(&select_str)?;
                    params.push(WithParam { name, select });
                    reader.read_to_end_into(e.name(), &mut Vec::new())?;
                }
                XmlEvent::Empty(e) if e.name().as_ref() == b"xsl:with-param" => {
                    let attrs = get_owned_attributes(&e)?;
                    let name = get_attr_owned_required(
                        &attrs,
                        b"name",
                        b"xsl:with-param",
                        pos,
                        full_xslt_str,
                    )?;
                    let select_str = get_attr_owned_required(
                        &attrs,
                        b"select",
                        b"xsl:with-param",
                        pos,
                        full_xslt_str,
                    )?;
                    let select = xpath::parse_selection(&select_str)?;
                    params.push(WithParam { name, select });
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                XmlEvent::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        Ok(params)
    }

    /// Helper function to resolve styles for a tag at compile time.
    fn resolve_styles(
        attributes: &OwnedAttributes,
        styles: &HashMap<String, Arc<ElementStyle>>,
        pos: usize,
        full_xslt_str: &str,
    ) -> Result<PreparsedStyles, ParseError> {
        let mut style_sets = Vec::new();
        let mut style_override = ElementStyle::default();
        let (line, col) = get_line_col_from_pos(full_xslt_str, pos);

        // Standard XSLT way to reference named styles.
        if let Some(names_str) = get_attr_owned_optional(attributes, b"use-attribute-sets")? {
            for name in names_str.split_whitespace() {
                let style_arc = styles.get(name).ok_or_else(|| ParseError::TemplateSyntax {
                    msg: format!("Attribute set '{}' not found.", name),
                    location: Location { line, col },
                })?;
                style_sets.push(Arc::clone(style_arc));
            }
        }

        // The `style` attribute can be used for either inline CSS or as a shorthand for `use-attribute-sets`.
        if let Some(style_attr_val) = get_attr_owned_optional(attributes, b"style")? {
            // Heuristic: if it contains CSS characters, treat as inline. Otherwise, treat as named styles.
            if style_attr_val.contains(':') || style_attr_val.contains(';') {
                style::parse_inline_css(&style_attr_val, &mut style_override)?;
            } else {
                for name in style_attr_val.split_whitespace() {
                    let style_arc = styles.get(name).ok_or_else(|| ParseError::TemplateSyntax {
                        msg: format!("Attribute set '{}' (from 'style' attribute) not found.", name),
                        location: Location { line, col },
                    })?;
                    style_sets.push(Arc::clone(style_arc));
                }
            }
        }

        // FO attributes are also treated as inline style overrides.
        style::parse_fo_attributes(attributes, &mut style_override)?;

        Ok(PreparsedStyles {
            style_sets,
            style_override: if style_override == ElementStyle::default() {
                None
            } else {
                Some(style_override)
            },
        })
    }

    /// Helper to get attributes that are not styling properties (e.g., `src`, `href`).
    fn get_non_style_attributes(
        attributes: &OwnedAttributes,
    ) -> Result<HashMap<String, String>, ParseError> {
        let mut attrs_map = HashMap::new();
        let style_keys = [
            "font-family",
            "font-size",
            "font-weight",
            "font-style",
            "line-height",
            "text-align",
            "color",
            "background-color",
            "border",
            "border-top",
            "border-bottom",
            "margin",
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
            "padding",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
            "width",
            "height",
            "style",
            "use-attribute-sets",
        ];

        for (key, value) in attributes {
            let key_str = String::from_utf8_lossy(key);
            if !style_keys.contains(&key_str.as_ref()) {
                attrs_map.insert(key_str.into_owned(), String::from_utf8(value.clone())?);
            }
        }
        Ok(attrs_map)
    }

    /// Special-cased compiler for `<table>`.
    fn compile_table(
        reader: &mut Reader<&[u8]>,
        end_qname: QName,
        styles: PreparsedStyles,
        style_map: &HashMap<String, Arc<ElementStyle>>,
        full_xslt_str: &str,
    ) -> Result<XsltInstruction, ParseError> {
        let mut columns = Vec::new();
        let mut header: Option<PreparsedTemplate> = None;
        let mut body_instructions = Vec::new();
        let mut buf = Vec::new();

        loop {
            let pos = reader.buffer_position() as usize;
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => {
                    let tag_name = e.name();
                    match tag_name.as_ref() {
                        b"columns" => columns = Self::compile_columns(reader, tag_name)?,
                        b"header" | b"fo:table-header" => {
                            header =
                                Some(Self::compile_body(reader, tag_name, style_map, full_xslt_str)?)
                        }
                        b"tbody" | b"fo:table-body" => {
                            let template =
                                Self::compile_body(reader, tag_name, style_map, full_xslt_str)?;
                            body_instructions.extend(template.0);
                        }
                        _ => {
                            let instruction = Self::compile_start_tag(
                                reader,
                                tag_name,
                                get_owned_attributes(&e)?,
                                style_map,
                                pos,
                                full_xslt_str,
                            )?;
                            body_instructions.push(instruction);
                        }
                    }
                }
                XmlEvent::Empty(e) => {
                    let instruction = Self::compile_empty_tag(&e, style_map, pos, full_xslt_str)?;
                    body_instructions.push(instruction);
                }
                XmlEvent::Text(e) => {
                    let text = e.unescape()?.into_owned();
                    if !text.trim().is_empty() {
                        body_instructions.push(XsltInstruction::Text(text));
                    }
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                XmlEvent::Eof => {
                    let (line, col) = get_line_col_from_pos(full_xslt_str, pos);
                    return Err(ParseError::TemplateSyntax {
                        msg: format!(
                            "Unexpected EOF while parsing table. Expected end tag for </{}>.",
                            String::from_utf8_lossy(end_qname.as_ref())
                        ),
                        location: Location { line, col },
                    });
                }
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
                XmlEvent::Empty(e)
                if e.name().as_ref() == b"column"
                    || e.name().as_ref() == b"fo:table-column" =>
                    {
                        let mut col = TableColumn::default();
                        for attr in e.attributes().flatten() {
                            let value = attr.decode_and_unescape_value(reader.decoder())?;
                            match attr.key.as_ref() {
                                b"column-width" | b"width" => {
                                    col.width = style::parse_dimension(&value).ok()
                                }
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