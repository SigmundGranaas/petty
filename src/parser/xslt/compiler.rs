use super::ast::{CompiledStylesheet, PreparsedStyles, PreparsedTemplate, TemplateRule, WithParam, XsltInstruction};
use super::util::{get_attr_owned_optional, get_attr_owned_required, get_line_col_from_pos, get_owned_attributes, OwnedAttributes};
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::ElementStyle;
use crate::parser::stylesheet_parser;
use crate::parser::{style, style_parsers, Location, ParseError};
use crate::xpath;
use handlebars::Handlebars;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::name::QName;
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

/// A stateful compiler that transforms an XSLT string into a `PreparsedTemplate` AST.
pub struct Compiler;

/// Returns a set of all tag names that the engine has specific logic for.
fn get_supported_tags() -> HashSet<&'static str> {
    [
        // XSLT control flow & data
        "xsl:for-each", "xsl:if", "xsl:value-of", "xsl:call-template", "xsl:apply-templates", "xsl:with-param", "xsl:param",
        // Block elements
        "page-sequence", "block", "fo:block", "p", "list", "fo:list-block", "list-item", "fo:list-item", "flex-container",
        "table", "fo:table", "header", "fo:table-header", "tbody", "fo:table-body", "row", "fo:table-row", "cell", "fo:table-cell",
        "columns", "column", "fo:table-column",
        // Inline elements
        "text", "span", "fo:inline", "strong", "b", "em", "i", "link", "fo:basic-link", "image", "fo:external-graphic", "br",
        // Custom control tags
        "page-break",
    ]
        .into()
}

impl Compiler {
    /// The main entry point for compiling an XSLT string. It performs a single pass,
    /// extracting styles and all templates into a structured `CompiledStylesheet`.
    pub fn compile(full_xslt_str: &str, resource_base_path: PathBuf) -> Result<CompiledStylesheet, ParseError> {
        // STEP 1: Use the robust XsltParser to extract all stylesheet definitions (styles, page masters).
        let stylesheet = stylesheet_parser::XsltParser::new(full_xslt_str).parse()?;

        // STEP 2: Now, parse only the template rules, using the styles we just extracted.
        let mut reader = Reader::from_str(full_xslt_str);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        let mut root_template = None;
        let mut template_rules: HashMap<Option<String>, Vec<TemplateRule>> = HashMap::new();
        let mut named_templates = HashMap::new();

        loop {
            let pos = reader.buffer_position() as u64;
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:template" => {
                    let attributes = get_owned_attributes(&e)?;
                    if let Some(match_attr) = get_attr_owned_optional(&attributes, b"match")? {
                        let body = Self::compile_body(&mut reader, e.name(), &stylesheet.styles, full_xslt_str)?;
                        if match_attr == "/" {
                            root_template = Some(body);
                        } else {
                            let mode = get_attr_owned_optional(&attributes, b"mode")?;
                            let priority = get_attr_owned_optional(&attributes, b"priority")?
                                .map(|p| p.parse::<f64>())
                                .transpose()
                                .map_err(|_| ParseError::TemplateSyntax {
                                    msg: "Invalid priority value".to_string(),
                                    location: get_line_col_from_pos(full_xslt_str, pos as usize).into(),
                                })?
                                .unwrap_or_else(|| Self::calculate_default_priority(&match_attr));

                            let rule = TemplateRule { match_pattern: match_attr, priority, mode: mode.clone(), body };
                            template_rules.entry(mode).or_default().push(rule);
                        }
                    } else if let Some(name) = get_attr_owned_optional(&attributes, b"name")? {
                        let body = Self::compile_body(&mut reader, e.name(), &stylesheet.styles, full_xslt_str)?;
                        named_templates.insert(name, body);
                    }
                }
                XmlEvent::Eof => break,
                _ => (),
            }
            buf.clear();
        }

        if root_template.is_none() {
            return Err(ParseError::TemplateStructure {
                message: "Could not find root <xsl:template match=\"/\">. This is required.".to_string(),
                location: Location { line: 0, col: 0 },
            });
        }

        // Sort all rule sets by priority (highest first)
        for rules in template_rules.values_mut() {
            rules.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
        }

        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(false);

        Ok(CompiledStylesheet {
            stylesheet,
            root_template,
            template_rules,
            named_templates,
            resource_base_path,
            handlebars,
        })
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
            let pos = reader.buffer_position() as u64;
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => {
                    instructions.push(Self::compile_start_tag(reader, e.name(), get_owned_attributes(&e)?, styles, pos as usize, full_xslt_str)?);
                }
                XmlEvent::Empty(e) => {
                    instructions.push(Self::compile_empty_tag(&e, styles, pos as usize, full_xslt_str)?);
                }
                XmlEvent::Text(e) => {
                    let text = e.unescape()?.into_owned();
                    if !text.trim().is_empty() {
                        instructions.push(XsltInstruction::Text(text));
                    }
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                XmlEvent::Eof => {
                    return Err(ParseError::TemplateSyntax {
                        msg: format!("Unexpected EOF. Expected end tag for </{}>.", String::from_utf8_lossy(end_qname.as_ref())),
                        location: get_line_col_from_pos(full_xslt_str, pos as usize).into(),
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
        let location = get_line_col_from_pos(full_xslt_str, pos).into();
        let tag_name_str = String::from_utf8_lossy(tag_name.as_ref());
        if !get_supported_tags().contains(tag_name_str.as_ref()) {
            log::warn!("Unsupported tag <{}> found at {}. Treating as generic block.", tag_name_str, location);
        }

        match tag_name.as_ref() {
            b"xsl:for-each" => {
                let select = xpath::parse_selection(&get_attr_owned_required(&attributes, b"select", b"xsl:for-each", pos, full_xslt_str)?)?;
                let body = Self::compile_body(reader, tag_name, styles, full_xslt_str)?;
                Ok(XsltInstruction::ForEach { select, body })
            }
            b"xsl:if" => {
                let test = xpath::parse_condition(&get_attr_owned_required(&attributes, b"test", b"xsl:if", pos, full_xslt_str)?)?;
                let body = Self::compile_body(reader, tag_name, styles, full_xslt_str)?;
                Ok(XsltInstruction::If { test, body })
            }
            b"xsl:call-template" => {
                let name = get_attr_owned_required(&attributes, b"name", b"xsl:call-template", pos, full_xslt_str)?;
                let params = Self::compile_with_params(reader, tag_name, full_xslt_str)?;
                Ok(XsltInstruction::CallTemplate { name, params })
            }
            b"xsl:param" => {
                reader.read_to_end_into(tag_name, &mut Vec::new())?;
                Ok(XsltInstruction::Text("".into()))
            }
            b"fo:table" | b"table" => {
                let preparsed_styles = Self::resolve_styles(&attributes, styles, location)?;
                Self::compile_table(reader, tag_name, preparsed_styles, styles, full_xslt_str)
            }
            _ => {
                let preparsed_styles = Self::resolve_styles(&attributes, styles, location)?;
                let attrs = Self::get_non_style_attributes(&attributes)?;
                let body = Self::compile_body(reader, tag_name, styles, full_xslt_str)?;
                Ok(XsltInstruction::ContentTag {
                    tag_name: tag_name.as_ref().to_vec(),
                    styles: preparsed_styles,
                    attrs,
                    body,
                })
            }
        }
    }

    /// Compiles an empty tag into the appropriate `XsltInstruction`.
    fn compile_empty_tag(e: &BytesStart, styles: &HashMap<String, Arc<ElementStyle>>, pos: usize, full_xslt_str: &str) -> Result<XsltInstruction, ParseError> {
        let attributes = get_owned_attributes(e)?;
        let location = get_line_col_from_pos(full_xslt_str, pos).into();
        let binding = e.name();
        let tag_name_str = String::from_utf8_lossy(binding.as_ref());
        if !get_supported_tags().contains(tag_name_str.as_ref()) {
            log::warn!("Unsupported empty tag <{}> at {}. Tag will be ignored.", tag_name_str, location);
        }

        match e.name().as_ref() {
            b"xsl:value-of" => {
                let select = xpath::parse_selection(&get_attr_owned_required(&attributes, b"select", b"xsl:value-of", pos, full_xslt_str)?)?;
                Ok(XsltInstruction::ValueOf { select })
            }
            b"xsl:apply-templates" => {
                let select = get_attr_owned_optional(&attributes, b"select")?.map(|s| xpath::parse_selection(&s)).transpose()?;
                let mode = get_attr_owned_optional(&attributes, b"mode")?;
                Ok(XsltInstruction::ApplyTemplates { select, mode })
            }
            b"xsl:with-param" | b"xsl:param" => Ok(XsltInstruction::Text("".into())),
            b"page-break" => {
                let master_name = get_attr_owned_optional(&attributes, b"master-name")?;
                Ok(XsltInstruction::PageBreak { master_name })
            }
            _ => {
                let preparsed_styles = Self::resolve_styles(&attributes, styles, location)?;
                let attrs = Self::get_non_style_attributes(&attributes)?;
                Ok(XsltInstruction::EmptyTag {
                    tag_name: e.name().as_ref().to_vec(),
                    styles: preparsed_styles,
                    attrs,
                })
            }
        }
    }

    /// Compiles the children of a `<xsl:call-template>` tag.
    fn compile_with_params(reader: &mut Reader<&[u8]>, end_qname: QName, full_xslt_str: &str) -> Result<Vec<WithParam>, ParseError> {
        let mut params = Vec::new();
        let mut buf = Vec::new();
        loop {
            let pos = reader.buffer_position() as u64;
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(ref e) | XmlEvent::Empty(ref e) if e.name().as_ref() == b"xsl:with-param" => {
                    let attrs = get_owned_attributes(e)?;
                    let name = get_attr_owned_required(&attrs, b"name", b"xsl:with-param", pos as usize, full_xslt_str)?;
                    let select_str = get_attr_owned_required(&attrs, b"select", b"xsl:with-param", pos as usize, full_xslt_str)?;
                    params.push(WithParam { name, select: xpath::parse_selection(&select_str)? });

                    let event_owned = e.to_owned();
                    if event_owned.name() == e.name() && !e.is_empty() {
                        reader.read_to_end_into(e.name(), &mut Vec::new())?;
                    }
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
    fn resolve_styles(attrs: &OwnedAttributes, styles: &HashMap<String, Arc<ElementStyle>>, location: Location) -> Result<PreparsedStyles, ParseError> {
        let mut style_sets = Vec::new();
        let mut style_override = ElementStyle::default();

        if let Some(names) = get_attr_owned_optional(attrs, b"use-attribute-sets")? {
            for name in names.split_whitespace() {
                style_sets.push(styles.get(name).cloned().ok_or_else(|| ParseError::TemplateSyntax {
                    msg: format!("Attribute set '{}' not found.", name),
                    location: location.clone(),
                })?);
            }
        }

        if let Some(val) = get_attr_owned_optional(attrs, b"style")? {
            if val.contains(':') || val.contains(';') {
                style::parse_inline_css(&val, &mut style_override)?;
            } else {
                for name in val.split_whitespace() {
                    style_sets.push(styles.get(name).cloned().ok_or_else(|| ParseError::TemplateSyntax {
                        msg: format!("Attribute set '{}' (from 'style' attr) not found.", name),
                        location: location.clone(),
                    })?);
                }
            }
        }

        style::parse_fo_attributes(attrs, &mut style_override)?;

        Ok(PreparsedStyles {
            style_sets,
            style_override: if style_override == ElementStyle::default() { None } else { Some(style_override) },
        })
    }

    /// Helper to get attributes that are not styling properties.
    fn get_non_style_attributes(attributes: &OwnedAttributes) -> Result<HashMap<String, String>, ParseError> {
        let style_keys = ["font-family", "font-size", "font-weight", "font-style", "line-height", "text-align", "color",
            "background-color", "border", "border-top", "border-bottom", "margin", "margin-top", "margin-right",
            "margin-bottom", "margin-left", "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
            "width", "height", "style", "use-attribute-sets"];
        attributes.iter()
            .filter_map(|(k, v)| {
                let key_str = String::from_utf8_lossy(k);
                if !style_keys.contains(&key_str.as_ref()) {
                    String::from_utf8(v.clone()).ok().map(|val| Ok((key_str.into_owned(), val)))
                } else { None }
            })
            .collect()
    }

    /// Special-cased compiler for `<table>`.
    fn compile_table(reader: &mut Reader<&[u8]>, end_qname: QName, styles: PreparsedStyles, style_map: &HashMap<String, Arc<ElementStyle>>, full_xslt_str: &str) -> Result<XsltInstruction, ParseError> {
        let mut columns = Vec::new();
        let mut header = None;
        let mut body_instructions = Vec::new();
        let mut buf = Vec::new();
        loop {
            let pos = reader.buffer_position() as u64;
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Start(e) => {
                    match e.name().as_ref() {
                        b"columns" => columns = Self::compile_columns(reader, e.name())?,
                        b"header" | b"fo:table-header" => header = Some(Self::compile_body(reader, e.name(), style_map, full_xslt_str)?),
                        b"tbody" | b"fo:table-body" => body_instructions.extend(Self::compile_body(reader, e.name(), style_map, full_xslt_str)?.0),
                        _ => body_instructions.push(Self::compile_start_tag(reader, e.name(), get_owned_attributes(&e)?, style_map, pos as usize, full_xslt_str)?),
                    }
                }
                XmlEvent::Empty(e) => body_instructions.push(Self::compile_empty_tag(&e, style_map, pos as usize, full_xslt_str)?),
                XmlEvent::Text(e) => {
                    let text = e.unescape()?.into_owned();
                    if !text.trim().is_empty() { body_instructions.push(XsltInstruction::Text(text)); }
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                XmlEvent::Eof => {
                    return Err(ParseError::TemplateSyntax {
                        msg: format!("Unexpected EOF while parsing table. Expected end tag for </{}>.", String::from_utf8_lossy(end_qname.as_ref())),
                        location: get_line_col_from_pos(full_xslt_str, pos as usize).into(),
                    });
                }
                _ => {}
            }
            buf.clear();
        }
        Ok(XsltInstruction::Table { styles, columns, header, body: PreparsedTemplate(body_instructions) })
    }

    /// Compiles the contents of a `<columns>` tag.
    fn compile_columns(reader: &mut Reader<&[u8]>, end_qname: QName) -> Result<Vec<Dimension>, ParseError> {
        let mut columns = Vec::new();
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::Empty(e) if e.name().as_ref() == b"column" || e.name().as_ref() == b"fo:table-column" => {
                    for attr in e.attributes().flatten() {
                        let value = attr.decode_and_unescape_value(reader.decoder())?;
                        if attr.key.as_ref() == b"column-width" || attr.key.as_ref() == b"width" {
                            if let Ok(dim) = style_parsers::run_parser(style_parsers::parse_dimension, &value) {
                                columns.push(dim);
                            }
                        }
                    }
                }
                XmlEvent::End(e) if e.name() == end_qname => break,
                _ => {}
            }
        }
        Ok(columns)
    }
}