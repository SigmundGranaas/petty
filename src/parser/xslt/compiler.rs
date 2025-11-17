//! Defines the CompilerBuilder, which constructs a `CompiledStylesheet` by listening to a parser driver.
use super::ast::{
    CompiledStylesheet, KeyDefinition, PreparsedStyles, PreparsedTemplate, When, XsltInstruction,
};
use super::parser;
use super::pattern;
use super::util::{get_attr_owned_optional, get_attr_owned_required, get_line_col_from_pos, OwnedAttributes};
use super::xpath;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::parser::processor::TemplateFlags;
use crate::parser::style;
use crate::parser::xslt::ast::{NamedTemplate, TemplateRule};
use crate::parser::ParseError;
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
pub(crate) enum BuilderState {
    Stylesheet,
    Template(OwnedAttributes),
    NamedTemplate {
        name: String,
        params: Vec<super::ast::Param>,
    },
    RoleTemplate {
        role_name: String,
        attrs: OwnedAttributes,
    },
    AttributeSet {
        name: String,
        style: ElementStyle,
    },
    Attribute(String), // The attribute name for an xsl:attribute-set
    InstructionBody(OwnedAttributes),
    XslText, // State for handling <xsl:text> content preservation
    Table {
        attrs: OwnedAttributes,
        columns: Vec<String>,
    },
    TableColumns,
    TableHeader,
    CallTemplate {
        name: String,
        params: Vec<super::ast::WithParam>,
    },
    Choose {
        whens: Vec<When>,
        otherwise: Option<PreparsedTemplate>,
    },
    When(OwnedAttributes),
    Otherwise,
    Sortable {
        attrs: OwnedAttributes,
        sort_keys: Vec<super::ast::SortKey>,
        saw_non_sort_child: bool,
    },
    // This is for `<xsl:attribute name="...">`, which generates an instruction.
    // It's different from `Attribute(String)` which is for `<xsl:attribute-set>`.
    InstructionAttribute(OwnedAttributes),
    // For `<xsl:element name="...">`
    InstructionElement(OwnedAttributes),
}

/// A stateful builder that constructs a `CompiledStylesheet` from parser events.
pub struct CompilerBuilder {
    pub(crate) stylesheet: Stylesheet,
    pub(crate) template_rules: HashMap<Option<String>, Vec<TemplateRule>>,
    pub(crate) named_templates: HashMap<String, Arc<NamedTemplate>>,
    pub(crate) role_template_modes: HashMap<String, String>,
    pub(crate) keys: Vec<KeyDefinition>,
    pub(crate) instruction_stack: Vec<Vec<XsltInstruction>>,
    pub(crate) state_stack: Vec<BuilderState>,
    pub(crate) features: TemplateFlags,
}

impl CompilerBuilder {
    fn new() -> Self {
        Self {
            stylesheet: Stylesheet::default(),
            template_rules: HashMap::new(),
            named_templates: HashMap::new(),
            role_template_modes: HashMap::new(),
            keys: Vec::new(),
            instruction_stack: vec![],
            state_stack: vec![BuilderState::Stylesheet],
            features: TemplateFlags::default(),
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
            stylesheet: Arc::new(self.stylesheet),
            template_rules: self.template_rules,
            named_templates: self.named_templates,
            keys: self.keys,
            resource_base_path,
            role_template_modes: self.role_template_modes,
            features: self.features,
        })
    }

    pub(crate) fn parse_xpath_and_detect_features(&mut self, expr_str: &str) -> Result<xpath::Expression, ParseError> {
        let expr = xpath::parse_expression(expr_str)?;
        if self.ast_contains_function(&expr, "petty:index") {
            self.features.uses_index_function = true;
        }
        Ok(expr)
    }

    fn ast_contains_function(&self, expr: &xpath::Expression, name: &str) -> bool {
        match expr {
            xpath::Expression::FunctionCall { name: func_name, args, .. } => {
                if func_name == name {
                    return true;
                }
                args.iter().any(|arg| self.ast_contains_function(arg, name))
            }
            xpath::Expression::BinaryOp { left, right, .. } => {
                self.ast_contains_function(left, name) || self.ast_contains_function(right, name)
            }
            xpath::Expression::UnaryOp { expr, .. } => {
                self.ast_contains_function(expr, name)
            }
            xpath::Expression::LocationPath(lp) => {
                let check_start = if let Some(sp) = &lp.start_point {
                    self.ast_contains_function(sp, name)
                } else { false };
                check_start || lp.steps.iter().any(|s| s.predicates.iter().any(|p| self.ast_contains_function(p, name)))
            }
            _ => false
        }
    }

    pub(crate) fn resolve_styles(&self, attrs: &OwnedAttributes, location: crate::parser::Location) -> Result<PreparsedStyles, ParseError> {
        let mut style_sets = Vec::new();
        let mut style_override = ElementStyle::default();

        let id = get_attr_owned_optional(attrs, b"id")?;

        // 1. Process `use-attribute-sets`
        if let Some(sets_str) = get_attr_owned_optional(attrs, b"use-attribute-sets")? {
            for set_name in sets_str.split_whitespace() {
                let style = self.stylesheet.styles.get(set_name).cloned().ok_or_else(|| {
                    ParseError::TemplateStructure {
                        message: format!("Attribute set '{}' not found", set_name),
                        location: location.clone(),
                    }
                })?;
                style_sets.push(style);
            }
        }

        // 2. Parse XSL-FO attributes directly on the element
        for (key, value) in attrs {
            let key_str = from_utf8(key)?;
            let value_str = from_utf8(value)?;
            style::apply_style_property(&mut style_override, key_str, value_str)?;
        }

        // 3. Parse inline `style` attribute, which overrides FO attributes
        if let Some(inline_style) = get_attr_owned_optional(attrs, b"style")? {
            style::parse_inline_css(&inline_style, &mut style_override)?;
        }

        Ok(PreparsedStyles {
            id,
            style_sets,
            style_override: if style_override == ElementStyle::default() {
                None
            } else {
                Some(style_override)
            },
        })
    }

    /// Handles parsing an <xsl:key> element.
    fn handle_key(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let name = get_attr_owned_required(&attrs, b"name", b"xsl:key", pos, source)?;
        let match_str = get_attr_owned_required(&attrs, b"match", b"xsl:key", pos, source)?;
        let use_str = get_attr_owned_required(&attrs, b"use", b"xsl:key", pos, source)?;

        let key_def = KeyDefinition {
            name,
            pattern: pattern::parse(&match_str)?,
            use_expr: self.parse_xpath_and_detect_features(&use_str)?,
        };
        self.keys.push(key_def);
        Ok(())
    }
}

impl StylesheetBuilder for CompilerBuilder {
    fn start_element(&mut self, e: &BytesStart, attrs: OwnedAttributes, pos: usize, source: &str) -> Result<(), ParseError> {
        let qname_binding = e.name();
        let name = qname_binding.as_ref();

        if let Some(BuilderState::Sortable { saw_non_sort_child, .. }) = self.state_stack.last_mut() {
            *saw_non_sort_child = true;
        }

        self.instruction_stack.push(Vec::new());

        match name {
            b"xsl:stylesheet" => self.handle_stylesheet_start(),
            b"xsl:template" => self.handle_template_start(attrs, pos, source)?,
            b"xsl:attribute-set" => self.handle_attribute_set_start(attrs, pos, source)?,
            b"xsl:attribute" => self.handle_attribute_start(attrs, pos, source)?,
            b"xsl:element" => self.handle_element_start(attrs),
            b"xsl:text" => self.handle_text_start(),
            b"fo:table" | b"table" => self.handle_table_start(attrs),
            b"columns" => self.handle_table_columns_start(),
            b"header" | b"fo:table-header" | b"thead" => self.handle_table_header_start(),
            b"xsl:call-template" => self.handle_call_template_start(attrs, pos, source)?,
            b"xsl:choose" => self.handle_choose_start(),
            b"xsl:when" => self.handle_when_start(attrs, pos, source)?,
            b"xsl:otherwise" => self.handle_otherwise_start(pos, source)?,
            b"xsl:for-each" | b"xsl:apply-templates" => self.handle_sortable_start(attrs),
            b"xsl:copy" => self.handle_copy_start(attrs),
            b"xsl:if" => self.state_stack.push(BuilderState::InstructionBody(attrs)),
            _ => self.handle_literal_result_element_start(attrs),
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

        match name {
            b"fo:simple-page-master" => self.handle_simple_page_master(attrs)?,
            b"xsl:key" => self.handle_key(attrs, pos, source)?,
            b"xsl:param" => self.handle_param(attrs, pos, source)?,
            b"xsl:with-param" => self.handle_with_param(attrs, pos, source)?,
            b"xsl:sort" => self.handle_sort(attrs, pos, source)?,
            b"xsl:value-of" => self.handle_value_of(attrs, pos, source)?,
            b"xsl:copy-of" => self.handle_copy_of(attrs, pos, source)?,
            b"xsl:variable" => self.handle_variable(attrs, pos, source)?,
            b"xsl:apply-templates" => self.handle_apply_templates_empty(attrs)?,
            b"page-break" => self.handle_page_break(attrs)?,
            b"toc" | b"fo:table-of-contents" => {
                self.features.has_table_of_contents = true;
                let instr = XsltInstruction::TableOfContents {
                    styles: self.resolve_styles(&attrs, location)?,
                };
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.push(instr);
                }
            }
            b"column" | b"fo:table-column" => self.handle_table_column(attrs)?,
            b"page-number-placeholder" => {
                self.features.has_page_number_placeholders = true;
                // This tag doesn't produce an instruction itself, it's just a marker.
            }
            _ => {
                // Handle literal result elements which are not XSLT instructions
                let instr = self.handle_empty_literal_result_element(e, attrs, location)?;
                if let Some(parent_body) = self.instruction_stack.last_mut() {
                    parent_body.push(instr);
                }
            }
        }
        Ok(())
    }

    fn end_element(&mut self, e: &BytesEnd, pos: usize, source: &str) -> Result<(), ParseError> {
        let qname_binding = e.name();
        let name = qname_binding.as_ref();
        let body = self.instruction_stack.pop().unwrap_or_default();
        let current_state = self.state_stack.pop().unwrap_or(BuilderState::Stylesheet);

        match name {
            b"xsl:stylesheet" => {}
            b"xsl:template" => self.handle_template_end(current_state, body, pos, source)?,
            b"xsl:attribute-set" => self.handle_attribute_set_end(current_state)?,
            b"xsl:attribute" => self.handle_attribute_end(current_state, body, pos, source)?,
            b"xsl:element" => self.handle_element_end(current_state, body, pos, source)?,
            b"xsl:text" => self.handle_text_end(body)?,
            b"xsl:if" => self.handle_if_end(current_state, body, pos, source)?,
            b"xsl:copy" => self.handle_copy_end(current_state, body, pos, source)?,
            b"fo:table" | b"table" => self.handle_table_end(current_state, body, pos, source)?,
            b"columns" | b"header" | b"fo:table-header" | b"thead" => {
                // These are handled by handle_table_end popping them from the state stack
            }
            b"xsl:call-template" => self.handle_call_template_end(current_state)?,
            b"xsl:when" => self.handle_when_end(current_state, body, pos, source)?,
            b"xsl:otherwise" => self.handle_otherwise_end(current_state, body, pos, source)?,
            b"xsl:choose" => self.handle_choose_end(current_state)?,
            b"xsl:for-each" => self.handle_for_each_end(current_state, body, pos, source)?,
            // An empty xsl:apply-templates is handled in empty_element. This handles a non-empty one.
            b"xsl:apply-templates" => { /* This is now handled by the generic sortable logic */ }
            b"page-number-placeholder" => {
                self.features.has_page_number_placeholders = true;
            }
            _ => self.handle_literal_result_element_end(e, current_state, body, pos, source)?,
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