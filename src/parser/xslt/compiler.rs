// FILE: src/parser/xslt/compiler.rs
//! Defines the CompilerBuilder, which constructs a `CompiledStylesheet` by listening to a parser driver.
use super::ast::{
    AttributeValueTemplate, CompiledStylesheet, NamedTemplate, Param, PreparsedStyles,
    PreparsedTemplate, SortDataType, SortKey, SortOrder, TemplateRule, When, WithParam,
    XsltInstruction,
};
use super::parser;
use super::pattern;
use super::util::{
    get_attr_owned_optional, get_attr_owned_required, get_line_col_from_pos, parse_avt,
    OwnedAttributes,
};
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use crate::parser::style::{apply_style_property, parse_page_size};
use crate::parser::style_parsers::{self, parse_length, parse_shorthand_margins, run_parser};
use crate::parser::{style, Location, ParseError};
use crate::parser::xpath;
use quick_xml::events::{BytesEnd, BytesStart};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::from_utf8;
use std::sync::Arc;

/// A trait defining the callbacks the parser driver will use to build a stylesheet.
pub trait StylesheetBuilder {
    fn start_element(&mut self, e: &BytesStart, attrs: OwnedAttributes, pos: usize, source: &str) -> Result<(), ParseError>;
    fn empty_element(&mut self, e: &BytesStart, attrs: OwnedAttributes, pos: usize, source: &str) -> Result<(), ParseError>;
    fn end_element(&mut self, e: &BytesEnd, pos: usize, source: &str) -> Result<(), ParseError>;
    fn text(&mut self, text: String) -> Result<(), ParseError>;
}

/// The main entry point for compiling an XSLT stylesheet.
pub fn compile(
    full_xslt_str: &str,
    resource_base_path: PathBuf,
) -> Result<CompiledStylesheet, ParseError> {
    let mut builder = CompilerBuilder::new();
    parser::parse_stylesheet_content(full_xslt_str, &mut builder)?;
    builder.finalize(resource_base_path)
}

/// Represents the current state of the builder, tracking nested structures.
enum BuilderState {
    Stylesheet,
    Template(OwnedAttributes),
    NamedTemplate {
        name: String,
        params: Vec<Param>,
    },
    AttributeSet {
        name: String,
        style: ElementStyle,
    },
    Attribute(String), // The attribute name for an xsl:attribute-set
    InstructionBody(OwnedAttributes),
    XslText, // State for handling <xsl:text> content preservation
    Table(OwnedAttributes),
    TableColumns,
    TableHeader,
    CallTemplate {
        name: String,
        params: Vec<WithParam>,
    },
    Choose {
        whens: Vec<When>,
        otherwise: Option<PreparsedTemplate>,
    },
    When(OwnedAttributes),
    Otherwise,
    Sortable {
        attrs: OwnedAttributes,
        sort_keys: Vec<SortKey>,
        saw_non_sort_child: bool,
    },
    // This is for `<xsl:attribute name="...">`, which generates an instruction.
    // It's different from `Attribute(String)` which is for `<xsl:attribute-set>`.
    InstructionAttribute(OwnedAttributes),
}

/// A stateful builder that constructs a `CompiledStylesheet` from parser events.
pub struct CompilerBuilder {
    stylesheet: Stylesheet,
    template_rules: HashMap<Option<String>, Vec<TemplateRule>>,
    named_templates: HashMap<String, Arc<NamedTemplate>>,
    instruction_stack: Vec<Vec<XsltInstruction>>,
    state_stack: Vec<BuilderState>,
}

impl CompilerBuilder {
    fn new() -> Self {
        Self {
            stylesheet: Stylesheet::default(),
            template_rules: HashMap::new(),
            named_templates: HashMap::new(),
            instruction_stack: vec![],
            state_stack: vec![BuilderState::Stylesheet],
        }
    }

    /// Consumes the builder to produce the final, compiled artifact.
    fn finalize(mut self, resource_base_path: PathBuf) -> Result<CompiledStylesheet, ParseError> {
        if self.stylesheet.default_page_master_name.is_none() {
            self.stylesheet.default_page_master_name = self.stylesheet.page_masters.keys().next().cloned();
        }

        for rules in self.template_rules.values_mut() {
            rules.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
        }

        Ok(CompiledStylesheet {
            stylesheet: self.stylesheet,
            template_rules: self.template_rules,
            named_templates: self.named_templates,
            resource_base_path,
        })
    }

    fn calculate_default_priority(pattern_str: &str) -> f64 {
        match pattern_str {
            "/" => -0.5,
            p if p.contains('*') => -0.5,
            p if p.contains('/') && !p.starts_with('/') => 0.0,
            p if p.contains(':') => 0.0,
            "text()" | "node()" => -0.25,
            _ => 0.0,
        }
    }
}

impl StylesheetBuilder for CompilerBuilder {
    fn start_element(&mut self, e: &BytesStart, attrs: OwnedAttributes, pos: usize, source: &str) -> Result<(), ParseError> {
        let qname_binding = e.name();
        let name = qname_binding.as_ref();
        let location = get_line_col_from_pos(source, pos).into();

        if let Some(BuilderState::Sortable { saw_non_sort_child, .. }) = self.state_stack.last_mut() {
            *saw_non_sort_child = true;
        }

        self.instruction_stack.push(Vec::new());

        match name {
            b"xsl:stylesheet" => self.state_stack.push(BuilderState::Stylesheet),
            b"xsl:template" => {
                if let Some(template_name) = get_attr_owned_optional(&attrs, b"name")? {
                    self.state_stack.push(BuilderState::NamedTemplate { name: template_name, params: vec![] });
                } else {
                    self.state_stack.push(BuilderState::Template(attrs));
                }
            },
            b"xsl:attribute-set" => {
                let name = get_attr_owned_required(&attrs, b"name", name, pos, source)?;
                self.state_stack.push(BuilderState::AttributeSet { name, style: ElementStyle::default() });
            },
            b"xsl:attribute" => {
                // This can be a direct child of attribute-set or an instruction.
                if matches!(self.state_stack.last(), Some(BuilderState::AttributeSet { .. })) {
                    let attr_name = get_attr_owned_required(&attrs, b"name", name, pos, source)?;
                    self.state_stack.push(BuilderState::Attribute(attr_name));
                } else {
                    self.state_stack.push(BuilderState::InstructionAttribute(attrs));
                }
            },
            b"xsl:text" => self.state_stack.push(BuilderState::XslText),
            b"fo:table" | b"table" => self.state_stack.push(BuilderState::Table(attrs)),
            b"columns" => self.state_stack.push(BuilderState::TableColumns),
            b"header" | b"fo:table-header" => self.state_stack.push(BuilderState::TableHeader),
            b"xsl:call-template" => {
                let name = get_attr_owned_required(&attrs, b"name", name, pos, source)?;
                self.state_stack.push(BuilderState::CallTemplate { name, params: vec![] });
            },
            b"xsl:choose" => {
                self.state_stack.push(BuilderState::Choose { whens: vec![], otherwise: None });
            },
            b"xsl:when" => {
                if !matches!(self.state_stack.last(), Some(BuilderState::Choose {..})) {
                    return Err(ParseError::TemplateStructure { message: "<xsl:when> must be a direct child of <xsl:choose>".to_string(), location });
                }
                self.state_stack.push(BuilderState::When(attrs));
            },
            b"xsl:otherwise" => {
                if !matches!(self.state_stack.last(), Some(BuilderState::Choose {..})) {
                    return Err(ParseError::TemplateStructure { message: "<xsl:otherwise> must be a direct child of <xsl:choose>".to_string(), location });
                }
                self.state_stack.push(BuilderState::Otherwise);
            },
            b"xsl:for-each" | b"xsl:apply-templates" => {
                self.state_stack.push(BuilderState::Sortable { attrs, sort_keys: vec![], saw_non_sort_child: false });
            },
            b"xsl:copy" => self.state_stack.push(BuilderState::InstructionBody(attrs)),
            _ => self.state_stack.push(BuilderState::InstructionBody(attrs)),
        }
        Ok(())
    }

    fn empty_element(&mut self, e: &BytesStart, attrs: OwnedAttributes, pos: usize, source: &str) -> Result<(), ParseError> {
        let qname_binding = e.name();
        let name = qname_binding.as_ref();
        let location = get_line_col_from_pos(source, pos).into();

        if name != b"xsl:sort" {
            if let Some(BuilderState::Sortable { saw_non_sort_child, .. }) = self.state_stack.last_mut() {
                *saw_non_sort_child = true;
            }
        }

        let instr = match name {
            b"fo:simple-page-master" => {
                let mut page = PageLayout::default();
                let master_name = get_attr_owned_optional(&attrs, b"master-name")?;
                for (key, val_bytes) in &attrs {
                    let key_str = from_utf8(key)?; let val_str = from_utf8(val_bytes)?;
                    match key_str {
                        "master-name" => {},
                        "page-width" => page.size.set_width(run_parser(parse_length, val_str)?),
                        "page-height" => page.size.set_height(run_parser(parse_length, val_str)?),
                        "size" => page.size = parse_page_size(val_str)?,
                        "margin" => page.margins = Some(parse_shorthand_margins(val_str)?),
                        _ => {}
                    }
                }
                self.stylesheet.page_masters.insert(master_name.unwrap_or_else(|| "default".to_string()), page);
                return Ok(());
            },
            b"xsl:param" => {
                if let Some(BuilderState::NamedTemplate { params, .. }) = self.state_stack.last_mut() {
                    let p_name = get_attr_owned_required(&attrs, b"name", name, pos, source)?;
                    let select = get_attr_owned_optional(&attrs, b"select")?.map(|s| xpath::parse_expression(&s)).transpose()?;
                    params.push(Param { name: p_name, default_value: select });
                } else {
                    return Err(ParseError::TemplateStructure{ message: "<xsl:param> can only appear at the top level of a named template.".to_string(), location });
                }
                return Ok(());
            },
            b"xsl:with-param" => {
                if let Some(BuilderState::CallTemplate { params, .. }) = self.state_stack.last_mut() {
                    let p_name = get_attr_owned_required(&attrs, b"name", name, pos, source)?;
                    let select = get_attr_owned_required(&attrs, b"select", name, pos, source)?;
                    let param = WithParam { name: p_name, select: xpath::parse_expression(&select)? };
                    params.push(param);
                }
                return Ok(());
            },
            b"xsl:sort" => {
                if let Some(BuilderState::Sortable { sort_keys, saw_non_sort_child, .. }) = self.state_stack.last_mut() {
                    if *saw_non_sort_child {
                        return Err(ParseError::TemplateStructure{ message: "<xsl:sort> must appear before any other content in its parent.".to_string(), location });
                    }
                    let select_str = get_attr_owned_optional(&attrs, b"select")?.unwrap_or_else(|| ".".to_string());
                    let order = match get_attr_owned_optional(&attrs, b"order")?.as_deref() {
                        Some("descending") => SortOrder::Descending,
                        _ => SortOrder::Ascending,
                    };
                    let data_type = match get_attr_owned_optional(&attrs, b"data-type")?.as_deref() {
                        Some("number") => SortDataType::Number,
                        _ => SortDataType::Text,
                    };
                    sort_keys.push(SortKey { select: xpath::parse_expression(&select_str)?, order, data_type });
                } else {
                    return Err(ParseError::TemplateStructure{ message: "<xsl:sort> can only appear inside <xsl:for-each> or <xsl:apply-templates>.".to_string(), location });
                }
                return Ok(());
            }
            b"xsl:value-of" => XsltInstruction::ValueOf { select: xpath::parse_expression(&get_attr_owned_required(&attrs, b"select", name, pos, source)?)? },
            b"xsl:copy-of" => XsltInstruction::CopyOf { select: xpath::parse_expression(&get_attr_owned_required(&attrs, b"select", name, pos, source)?)? },
            b"xsl:variable" => XsltInstruction::Variable { name: get_attr_owned_required(&attrs, b"name", name, pos, source)?, select: xpath::parse_expression(&get_attr_owned_required(&attrs, b"select", name, pos, source)?)? },
            b"xsl:apply-templates" => XsltInstruction::ApplyTemplates { select: get_attr_owned_optional(&attrs, b"select")?.map(|s| xpath::parse_expression(&s)).transpose()?, mode: get_attr_owned_optional(&attrs, b"mode")?, sort_keys: vec![] },
            b"page-break" => XsltInstruction::PageBreak { master_name: get_attr_owned_optional(&attrs, b"master-name")? },
            b"column" | b"fo:table-column" => {
                if let Some(BuilderState::TableColumns) = self.state_stack.last() {
                    let width = get_attr_owned_optional(&attrs, b"column-width")?.or(get_attr_owned_optional(&attrs, b"width")?);
                    if let Some(w_str) = width {
                        if let Some(instrs) = self.instruction_stack.last_mut() {
                            // Bit of a hack: store dimensions as dummy text instructions
                            instrs.push(XsltInstruction::Text(w_str));
                        }
                    }
                }
                return Ok(());
            }
            _ => {
                let styles = resolve_styles(&attrs, &self.stylesheet.styles, location)?;
                let non_style_attrs = get_non_style_attributes(&attrs)?;
                XsltInstruction::EmptyTag { tag_name: e.name().as_ref().to_vec(), styles, attrs: non_style_attrs }
            }
        };

        if let Some(parent_body) = self.instruction_stack.last_mut() {
            parent_body.push(instr);
        }
        Ok(())
    }

    fn end_element(&mut self, e: &BytesEnd, pos: usize, source: &str) -> Result<(), ParseError> {
        let qname_binding = e.name();
        let name = qname_binding.as_ref();
        let body = self.instruction_stack.pop().unwrap_or_default();
        let current_state = self.state_stack.pop().unwrap_or(BuilderState::Stylesheet);
        let location = get_line_col_from_pos(source, pos).into();

        match current_state {
            BuilderState::Stylesheet => {},
            BuilderState::Template(attrs) => {
                let match_str = get_attr_owned_required(&attrs, b"match", name, pos, source)?;
                let pattern = pattern::parse(&match_str)?;
                let mode = get_attr_owned_optional(&attrs, b"mode")?;
                let priority = get_attr_owned_optional(&attrs, b"priority")?.map(|p_str| p_str.parse::<f64>().map_err(|e| ParseError::FloatParse(e, p_str.clone()))).transpose()?.unwrap_or_else(|| Self::calculate_default_priority(&match_str));
                let rule = TemplateRule { pattern, priority, mode: mode.clone(), body: PreparsedTemplate(body) };
                self.template_rules.entry(mode).or_default().push(rule);
            },
            BuilderState::NamedTemplate { name, params } => {
                let template = NamedTemplate { params, body: PreparsedTemplate(body) };
                self.named_templates.insert(name, Arc::new(template));
            }
            BuilderState::AttributeSet { name, style } => {
                // When the attribute set ends, we have already processed all its children
                // and the style object is complete.
                self.stylesheet.styles.insert(name, Arc::new(style));
            },
            BuilderState::Attribute(prop) => {
                let value = body.iter().find_map(|i| if let XsltInstruction::Text(t) = i { Some(t.clone()) } else { None }).unwrap_or_default();
                if let Some(BuilderState::AttributeSet { style, .. }) = self.state_stack.last_mut() {
                    apply_style_property(style, &prop, &value)?;
                }
            }
            BuilderState::XslText => {
                // The body contains the generated text instruction(s).
                // Merge them into the parent's instruction list.
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.extend(body);
                }
            }
            BuilderState::InstructionBody(attrs) => {
                let instr = match name {
                    b"xsl:if" => XsltInstruction::If { test: xpath::parse_expression(&get_attr_owned_required(&attrs, b"test", name, pos, source)?)?, body: PreparsedTemplate(body) },
                    b"xsl:copy" => XsltInstruction::Copy { styles: resolve_styles(&attrs, &self.stylesheet.styles, location)?, body: PreparsedTemplate(body) },
                    _ => {
                        let styles = resolve_styles(&attrs, &self.stylesheet.styles, location)?;
                        let non_style_attrs = get_non_style_attributes(&attrs)?;
                        XsltInstruction::ContentTag { tag_name: e.name().as_ref().to_vec(), styles, attrs: non_style_attrs, body: PreparsedTemplate(body) }
                    }
                };
                if let Some(parent) = self.instruction_stack.last_mut() { parent.push(instr); }
            },
            BuilderState::InstructionAttribute(attrs) => {
                let attr_name = get_attr_owned_required(&attrs, b"name", name, pos, source)?;
                let instr = XsltInstruction::Attribute { name: attr_name, body: PreparsedTemplate(body) };
                if let Some(parent) = self.instruction_stack.last_mut() { parent.push(instr); }
            }
            BuilderState::Table(attrs) => {
                let mut header = None;
                let mut columns = Vec::new();

                if let Some(BuilderState::TableHeader) = self.state_stack.last() {
                    self.state_stack.pop();
                    header = Some(PreparsedTemplate(self.instruction_stack.pop().unwrap_or_default()));
                }
                if let Some(BuilderState::TableColumns) = self.state_stack.last() {
                    self.state_stack.pop();
                    let col_body = self.instruction_stack.pop().unwrap_or_default();
                    for instr in col_body {
                        if let XsltInstruction::Text(dim_str) = instr {
                            columns.push(run_parser(style_parsers::parse_dimension, &dim_str)?);
                        }
                    }
                }
                let styles = resolve_styles(&attrs, &self.stylesheet.styles, location)?;
                let table_instr = XsltInstruction::Table { styles, columns, header, body: PreparsedTemplate(body) };
                if let Some(parent) = self.instruction_stack.last_mut() { parent.push(table_instr); }
            },
            BuilderState::CallTemplate { name, params } => {
                let instr = XsltInstruction::CallTemplate { name, params };
                if let Some(parent) = self.instruction_stack.last_mut() { parent.push(instr); }
            },
            BuilderState::When(attrs) => {
                let test = xpath::parse_expression(&get_attr_owned_required(&attrs, b"test", name, pos, source)?)?;
                let when_block = When { test, body: PreparsedTemplate(body) };
                if let Some(BuilderState::Choose { whens, .. }) = self.state_stack.last_mut() {
                    whens.push(when_block);
                } else {
                    return Err(ParseError::TemplateStructure { message: "Internal compiler error: <xsl:when> not inside <xsl:choose>.".to_string(), location });
                }
            },
            BuilderState::Otherwise => {
                if let Some(BuilderState::Choose { otherwise, .. }) = self.state_stack.last_mut() {
                    if otherwise.is_some() {
                        return Err(ParseError::TemplateStructure { message: "Only one <xsl:otherwise> is allowed inside <xsl:choose>".to_string(), location });
                    }
                    *otherwise = Some(PreparsedTemplate(body));
                } else {
                    return Err(ParseError::TemplateStructure { message: "Internal compiler error: <xsl:otherwise> not inside <xsl:choose>.".to_string(), location });
                }
            },
            BuilderState::Choose { whens, otherwise } => {
                let instr = XsltInstruction::Choose { whens, otherwise };
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.push(instr);
                }
            },
            BuilderState::Sortable { attrs, sort_keys, .. } => {
                let instr = match name {
                    b"xsl:for-each" => XsltInstruction::ForEach {
                        select: xpath::parse_expression(&get_attr_owned_required(&attrs, b"select", name, pos, source)?)?,
                        sort_keys,
                        body: PreparsedTemplate(body),
                    },
                    b"xsl:apply-templates" => XsltInstruction::ApplyTemplates {
                        select: get_attr_owned_optional(&attrs, b"select")?.map(|s| xpath::parse_expression(&s)).transpose()?,
                        mode: get_attr_owned_optional(&attrs, b"mode")?,
                        sort_keys,
                    },
                    _ => unreachable!(),
                };
                if let Some(parent) = self.instruction_stack.last_mut() { parent.push(instr); }
            }
            _ => {}
        }
        Ok(())
    }

    fn text(&mut self, text: String) -> Result<(), ParseError> {
        if let Some(BuilderState::Sortable { saw_non_sort_child, .. }) = self.state_stack.last_mut() {
            if !text.trim().is_empty() {
                *saw_non_sort_child = true;
            }
        }
        let is_in_xsl_text = matches!(self.state_stack.last(), Some(BuilderState::XslText));

        // Preserve whitespace content if it's from <xsl:text>.
        // Otherwise, only keep text that has non-whitespace characters.
        if is_in_xsl_text || !text.trim().is_empty() {
            if let Some(body) = self.instruction_stack.last_mut() {
                body.push(XsltInstruction::Text(text));
            }
        }
        Ok(())
    }
}
// Helper functions (could be moved to util)
fn resolve_styles(attrs: &OwnedAttributes, styles: &HashMap<String, Arc<ElementStyle>>, location: Location) -> Result<PreparsedStyles, ParseError> {
    let mut style_sets = Vec::new();
    let mut style_override = ElementStyle::default();
    if let Some(names) = get_attr_owned_optional(attrs, b"use-attribute-sets")? {
        for name in names.split_whitespace() { style_sets.push(styles.get(name).cloned().ok_or_else(|| ParseError::TemplateSyntax { msg: format!("Attribute set '{}' not found.", name), location: location.clone() })?); }
    }
    style::parse_fo_attributes(attrs, &mut style_override)?;
    Ok(PreparsedStyles { style_sets, style_override: if style_override == ElementStyle::default() { None } else { Some(style_override) } })
}

fn get_non_style_attributes(
    attributes: &OwnedAttributes,
) -> Result<HashMap<String, AttributeValueTemplate>, ParseError> {
    attributes
        .iter()
        .filter_map(|(k, v)| {
            let key_str = String::from_utf8_lossy(k);
            if !key_str.starts_with("font-")
                && !key_str.starts_with("margin-")
                && !key_str.starts_with("padding-")
                && key_str != "style"
                && key_str != "use-attribute-sets"
            {
                let res = from_utf8(v)
                    .map_err(ParseError::from)
                    .and_then(|val_str| parse_avt(val_str))
                    .map(|avt| (key_str.into_owned(), avt));
                Some(res)
            } else {
                None
            }
        })
        .collect()
}