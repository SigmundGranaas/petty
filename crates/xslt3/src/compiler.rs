//! XSLT 3.0 compiler that builds a `CompiledStylesheet3` from XML events.
//!
//! This extends the XSLT 1.0 compiler with support for:
//! - Text value templates (expand-text)
//! - xsl:try/xsl:catch for error handling
//! - xsl:iterate for streaming iteration
//! - xsl:map, xsl:map-entry, xsl:array, xsl:array-member
//! - xsl:fork, xsl:merge for parallel/streaming processing
//! - xsl:assert, xsl:message enhancements
//! - xsl:on-empty, xsl:on-non-empty, xsl:where-populated
//! - accumulators

use crate::ast::{
    Accumulator, AccumulatorPhase, AccumulatorRule, CatchClause, CompiledStylesheet3, ForkBranch,
    Function3, GlobalParam, GlobalVariable, IterateParam, KeyDeclaration, MergeAction, MergeKey,
    MergeSource, ModeDeclaration, NamedTemplate3, OccurrenceIndicator, OutputDeclaration, Param3,
    Pattern3, PreparsedTemplate, SequenceType, SortKey3, TemplateRule3, TextValueTemplate, TvtPart,
    UsePackage, Visibility, When3, WithParam3, Xslt3Features, Xslt3Instruction,
};
use crate::error::Xslt3Error;
use petty_style::parsers as style;
use petty_style::stylesheet::ElementStyle;
use petty_xslt::ast::PreparsedStyles;
use quick_xml::events::{BytesEnd, BytesStart};
use std::collections::HashMap;
use std::str::from_utf8;
use std::sync::Arc;

/// Owned attributes from XML parsing
pub type OwnedAttributes = Vec<(Vec<u8>, Vec<u8>)>;

/// A trait defining the callbacks the parser driver will use to build a stylesheet.
pub trait StylesheetBuilder3 {
    fn start_element(
        &mut self,
        e: &BytesStart,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error>;
    fn empty_element(
        &mut self,
        e: &BytesStart,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error>;
    fn end_element(&mut self, e: &BytesEnd, pos: usize, source: &str) -> Result<(), Xslt3Error>;
    fn text(&mut self, text: String) -> Result<(), Xslt3Error>;
}

/// Represents the current state of the builder, tracking nested structures.
#[derive(Debug)]
pub enum BuilderState3 {
    Stylesheet,
    Template(OwnedAttributes),
    NamedTemplate {
        name: String,
        params: Vec<Param3>,
    },
    Function {
        name: String,
        params: Vec<Param3>,
        as_type: Option<SequenceType>,
    },
    Accumulator {
        name: String,
        initial_value: Option<petty_xpath31::Expression>,
        rules: Vec<AccumulatorRule>,
        streamable: bool,
    },
    AccumulatorRule {
        pattern: Pattern3,
        phase: AccumulatorPhase,
    },
    InstructionBody(OwnedAttributes),
    XslText,
    // XSLT 3.0 specific states
    Try {
        body_instructions: Vec<Xslt3Instruction>,
        catches: Vec<CatchClause>,
        rollback_output: bool,
    },
    Catch {
        errors: Vec<String>,
    },
    Iterate {
        select: petty_xpath31::Expression,
        params: Vec<IterateParam>,
        body_instructions: Vec<Xslt3Instruction>,
        on_completion: Option<PreparsedTemplate>,
    },
    OnCompletion,
    Map {
        entries: Vec<crate::ast::MapEntryInstruction>,
    },
    MapEntry {
        key: petty_xpath31::Expression,
    },
    Array {
        members: Vec<crate::ast::ArrayMemberInstruction>,
    },
    ArrayMember,
    Fork {
        branches: Vec<ForkBranch>,
    },
    Merge {
        sources: Vec<MergeSource>,
        action: Option<MergeAction>,
    },
    MergeSource {
        name: Option<String>,
        for_each_item: Option<petty_xpath31::Expression>,
        select: petty_xpath31::Expression,
        sort_keys: Vec<MergeKey>,
        streamable: bool,
    },
    MergeAction,
    Choose {
        whens: Vec<When3>,
        otherwise: Option<PreparsedTemplate>,
    },
    When(OwnedAttributes),
    Otherwise,
    Sortable {
        attrs: OwnedAttributes,
        sort_keys: Vec<SortKey3>,
        saw_non_sort_child: bool,
    },
    CallTemplate {
        name: String,
        params: Vec<WithParam3>,
    },
    SourceDocument {
        href: petty_xslt::ast::AttributeValueTemplate,
        streamable: bool,
    },
    ResultDocument {
        format: Option<String>,
        href: Option<petty_xslt::ast::AttributeValueTemplate>,
    },
    Message {
        select: Option<petty_xpath31::Expression>,
        terminate: bool,
        error_code: Option<String>,
    },
    Assert {
        test: petty_xpath31::Expression,
    },
    NextIteration {
        params: Vec<crate::ast::NextIterationParam>,
    },
    AnalyzeString {
        select: petty_xpath31::Expression,
        regex: String,
        flags: Option<String>,
        matching_substring: Option<PreparsedTemplate>,
        non_matching_substring: Option<PreparsedTemplate>,
    },
    MatchingSubstring,
    AttributeSet {
        name: String,
        use_attribute_sets: Vec<String>,
        visibility: crate::ast::Visibility,
    },
    NonMatchingSubstring,
    PerformSort {
        select: Option<petty_xpath31::Expression>,
        sort_keys: Vec<SortKey3>,
    },
    Variable {
        name: String,
        select: Option<petty_xpath31::Expression>,
        as_type: Option<SequenceType>,
    },
    CharacterMap {
        name: String,
        use_character_maps: Vec<String>,
        mappings: Vec<crate::ast::OutputCharacter>,
    },
}

pub struct CompilerBuilder3 {
    pub(crate) version: String,
    pub(crate) default_mode: Option<String>,
    pub(crate) expand_text: bool,
    pub(crate) expand_text_stack: Vec<bool>,
    pub(crate) use_packages: Vec<UsePackage>,
    pub(crate) imports: Vec<crate::ast::ImportDeclaration>,
    pub(crate) includes: Vec<crate::ast::IncludeDeclaration>,
    pub(crate) global_variables: HashMap<String, GlobalVariable>,
    pub(crate) global_params: HashMap<String, GlobalParam>,
    pub(crate) template_rules: HashMap<Option<String>, Vec<TemplateRule3>>,
    pub(crate) named_templates: HashMap<String, Arc<NamedTemplate3>>,
    pub(crate) functions: HashMap<String, Arc<Function3>>,
    pub(crate) accumulators: HashMap<String, Accumulator>,
    pub(crate) keys: HashMap<String, KeyDeclaration>,
    pub(crate) attribute_sets: HashMap<String, crate::ast::AttributeSet>,
    pub(crate) output: OutputDeclaration,
    pub(crate) outputs: HashMap<String, OutputDeclaration>,
    pub(crate) decimal_formats: HashMap<Option<String>, crate::ast::DecimalFormatDeclaration>,
    pub(crate) namespace_aliases: Vec<crate::ast::NamespaceAlias>,
    pub(crate) character_maps: HashMap<String, crate::ast::CharacterMap>,
    pub(crate) preserve_space: Vec<String>,
    pub(crate) strip_space: Vec<String>,
    pub(crate) features: Xslt3Features,
    pub(crate) instruction_stack: Vec<Vec<Xslt3Instruction>>,
    pub(crate) state_stack: Vec<BuilderState3>,
    pub(crate) stylesheet: petty_style::stylesheet::Stylesheet,
    pub(crate) modes: HashMap<Option<String>, ModeDeclaration>,
    pub(crate) context_item: Option<crate::ast::ContextItemDeclaration>,
    pub(crate) global_context_item: Option<crate::ast::GlobalContextItemDeclaration>,
    pub(crate) initial_template: Option<crate::ast::InitialTemplateDeclaration>,
}

impl Default for CompilerBuilder3 {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerBuilder3 {
    pub fn new() -> Self {
        Self {
            version: "3.0".to_string(),
            default_mode: None,
            expand_text: false,
            expand_text_stack: vec![false],
            use_packages: Vec::new(),
            imports: Vec::new(),
            includes: Vec::new(),
            global_variables: HashMap::new(),
            global_params: HashMap::new(),
            template_rules: HashMap::new(),
            named_templates: HashMap::new(),
            functions: HashMap::new(),
            accumulators: HashMap::new(),
            keys: HashMap::new(),
            attribute_sets: HashMap::new(),
            output: OutputDeclaration::default(),
            outputs: HashMap::new(),
            decimal_formats: HashMap::new(),
            namespace_aliases: Vec::new(),
            character_maps: HashMap::new(),
            features: Xslt3Features::default(),
            instruction_stack: vec![],
            state_stack: vec![BuilderState3::Stylesheet],
            stylesheet: petty_style::stylesheet::Stylesheet::default(),
            modes: HashMap::new(),
            context_item: None,
            global_context_item: None,
            initial_template: None,
            preserve_space: Vec::new(),
            strip_space: Vec::new(),
        }
    }

    pub fn finalize(mut self) -> Result<CompiledStylesheet3, Xslt3Error> {
        for rules in self.template_rules.values_mut() {
            rules.sort_by(|a, b| {
                b.priority
                    .partial_cmp(&a.priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(CompiledStylesheet3 {
            version: self.version,
            default_mode: self.default_mode,
            expand_text: self.expand_text,
            use_packages: self.use_packages,
            imports: self.imports,
            includes: self.includes,
            global_variables: self.global_variables,
            global_params: self.global_params,
            template_rules: self.template_rules,
            named_templates: self.named_templates,
            functions: self.functions,
            accumulators: self.accumulators,
            keys: self.keys,
            attribute_sets: self.attribute_sets,
            output: self.output,
            outputs: self.outputs,
            decimal_formats: self.decimal_formats,
            namespace_aliases: self.namespace_aliases,
            character_maps: self.character_maps,
            features: self.features,
            stylesheet: self.stylesheet,
            modes: self.modes,
            context_item: self.context_item,
            global_context_item: self.global_context_item,
            initial_template: self.initial_template,
            preserve_space: self.preserve_space,
            strip_space: self.strip_space,
        })
    }

    /// Get the current expand-text setting
    fn current_expand_text(&self) -> bool {
        *self.expand_text_stack.last().unwrap_or(&self.expand_text)
    }

    /// Push expand-text setting from element attributes
    fn push_expand_text(&mut self, attrs: &OwnedAttributes) {
        let current = self.current_expand_text();
        let new_value = get_attr_optional(attrs, b"expand-text")
            .ok()
            .flatten()
            .map(|v| v == "yes" || v == "true")
            .unwrap_or(current);
        self.expand_text_stack.push(new_value);
    }

    /// Pop expand-text setting
    fn pop_expand_text(&mut self) {
        self.expand_text_stack.pop();
    }

    pub(crate) fn parse_xpath(
        &mut self,
        expr_str: &str,
    ) -> Result<petty_xpath31::Expression, Xslt3Error> {
        petty_xpath31::parse_expression(expr_str).map_err(|e| {
            Xslt3Error::parse(format!(
                "Failed to parse XPath expression '{}': {}",
                expr_str, e
            ))
        })
    }

    /// Parse a text value template into parts
    pub(crate) fn parse_tvt(&mut self, text: &str) -> Result<TextValueTemplate, Xslt3Error> {
        let mut parts = Vec::new();
        let mut current_static = String::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        // Escaped {{
                        chars.next();
                        current_static.push('{');
                    } else {
                        // Start of expression
                        if !current_static.is_empty() {
                            parts.push(TvtPart::Static(std::mem::take(&mut current_static)));
                        }
                        // Collect expression until }
                        let mut expr_str = String::new();
                        let mut depth = 1;
                        for ec in chars.by_ref() {
                            match ec {
                                '{' => {
                                    depth += 1;
                                    expr_str.push(ec);
                                }
                                '}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                    expr_str.push(ec);
                                }
                                _ => expr_str.push(ec),
                            }
                        }
                        let expr = self.parse_xpath(&expr_str)?;
                        parts.push(TvtPart::Dynamic(expr));
                    }
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        // Escaped }}
                        chars.next();
                        current_static.push('}');
                    } else {
                        // Lone } is an error in strict mode, but we'll be lenient
                        current_static.push('}');
                    }
                }
                _ => current_static.push(c),
            }
        }

        if !current_static.is_empty() {
            parts.push(TvtPart::Static(current_static));
        }

        Ok(TextValueTemplate(parts))
    }

    pub(crate) fn resolve_styles(
        &self,
        attrs: &OwnedAttributes,
    ) -> Result<PreparsedStyles, Xslt3Error> {
        let mut style_sets = Vec::new();
        let mut style_override = ElementStyle::default();

        let id = get_attr_optional(attrs, b"id")?;

        if let Some(sets_str) = get_attr_optional(attrs, b"use-attribute-sets")? {
            for set_name in sets_str.split_whitespace() {
                if let Some(s) = self.stylesheet.styles.get(set_name) {
                    style_sets.push(s.clone());
                }
            }
        }

        for (key, value) in attrs {
            let key_str = from_utf8(key).map_err(|e| Xslt3Error::parse(e.to_string()))?;
            let value_str = from_utf8(value).map_err(|e| Xslt3Error::parse(e.to_string()))?;
            let _ = style::apply_style_property(&mut style_override, key_str, value_str);
        }

        if let Some(inline_style) = get_attr_optional(attrs, b"style")? {
            let _ = style::parse_inline_css(&inline_style, &mut style_override);
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

    /// Parse a sequence type from string
    pub(crate) fn parse_sequence_type(&self, s: &str) -> Option<SequenceType> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let (item_type, occurrence) = if let Some(stripped) = s.strip_suffix('?') {
            (stripped.to_string(), OccurrenceIndicator::ZeroOrOne)
        } else if let Some(stripped) = s.strip_suffix('*') {
            (stripped.to_string(), OccurrenceIndicator::ZeroOrMore)
        } else if let Some(stripped) = s.strip_suffix('+') {
            (stripped.to_string(), OccurrenceIndicator::OneOrMore)
        } else {
            (s.to_string(), OccurrenceIndicator::ExactlyOne)
        };

        Some(SequenceType {
            item_type,
            occurrence,
        })
    }

    /// Parse visibility from string
    pub(crate) fn parse_visibility(&self, s: &str) -> Visibility {
        match s.to_lowercase().as_str() {
            "public" => Visibility::Public,
            "final" => Visibility::Final,
            "abstract" => Visibility::Abstract,
            _ => Visibility::Private,
        }
    }
}

impl StylesheetBuilder3 for CompilerBuilder3 {
    fn start_element(
        &mut self,
        e: &BytesStart,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name_binding = e.name();
        let name = name_binding.as_ref();
        self.push_expand_text(&attrs);

        if let Some(BuilderState3::Sortable {
            saw_non_sort_child, ..
        }) = self.state_stack.last_mut()
        {
            *saw_non_sort_child = true;
        }

        if name == b"xsl:catch"
            && let Some(BuilderState3::Try {
                body_instructions, ..
            }) = self.state_stack.last_mut()
        {
            let body = self.instruction_stack.pop().unwrap_or_default();
            body_instructions.extend(body);
        }

        self.instruction_stack.push(Vec::new());

        match name {
            b"xsl:stylesheet" | b"xsl:transform" => {
                self.handle_stylesheet_start(&attrs)?;
            }
            b"xsl:package" => {
                self.handle_package_start(&attrs)?;
            }
            b"xsl:import" => {
                self.handle_import(&attrs, pos, source)?;
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
            b"xsl:include" => {
                self.handle_include(&attrs, pos, source)?;
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
            b"xsl:template" => {
                self.handle_template_start(attrs, pos, source)?;
            }
            b"xsl:function" => {
                self.handle_function_start(attrs, pos, source)?;
            }
            b"xsl:accumulator" => {
                self.handle_accumulator_start(attrs, pos, source)?;
            }
            b"xsl:key" => {
                self.handle_key(&attrs, pos, source)?;
            }
            b"xsl:attribute-set" => {
                self.handle_attribute_set_start(&attrs, pos, source)?;
            }
            b"xsl:character-map" => {
                self.handle_character_map_start(&attrs, pos, source)?;
            }
            b"xsl:accumulator-rule" => {
                self.handle_accumulator_rule_start(attrs, pos, source)?;
            }
            b"xsl:try" => {
                self.handle_try_start(&attrs)?;
            }
            b"xsl:catch" => {
                self.handle_catch_start(&attrs)?;
            }
            b"xsl:iterate" => {
                self.handle_iterate_start(attrs, pos, source)?;
            }
            b"xsl:on-completion" => {
                self.state_stack.push(BuilderState3::OnCompletion);
            }
            b"xsl:map" => {
                self.features.uses_maps = true;
                self.state_stack.push(BuilderState3::Map {
                    entries: Vec::new(),
                });
            }
            b"xsl:map-entry" => {
                self.handle_map_entry_start(attrs, pos, source)?;
            }
            b"xsl:array" => {
                self.features.uses_arrays = true;
                self.state_stack.push(BuilderState3::Array {
                    members: Vec::new(),
                });
            }
            b"xsl:array-member" => {
                self.state_stack.push(BuilderState3::ArrayMember);
            }
            b"xsl:fork" => {
                self.features.uses_fork = true;
                self.state_stack.push(BuilderState3::Fork {
                    branches: Vec::new(),
                });
            }
            b"xsl:sequence" => {
                // Just push as instruction body
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
            b"xsl:merge" => {
                self.handle_merge_start()?;
            }
            b"xsl:merge-source" => {
                self.handle_merge_source_start(attrs, pos, source)?;
            }
            b"xsl:merge-action" => {
                self.state_stack.push(BuilderState3::MergeAction);
            }
            b"xsl:merge-key" => {
                self.handle_merge_key(&attrs, pos, source)?;
            }
            b"xsl:source-document" => {
                self.handle_source_document_start(attrs, pos, source)?;
            }
            b"xsl:result-document" => {
                self.handle_result_document_start(&attrs)?;
            }
            b"xsl:stream" => {
                self.features.uses_streaming = true;
                self.handle_stream_start(attrs, pos, source)?;
            }
            b"xsl:text" => {
                self.state_stack.push(BuilderState3::XslText);
            }
            b"xsl:if" => {
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
            b"xsl:choose" => {
                self.state_stack.push(BuilderState3::Choose {
                    whens: Vec::new(),
                    otherwise: None,
                });
            }
            b"xsl:when" => {
                self.handle_when_start(attrs, pos, source)?;
            }
            b"xsl:otherwise" => {
                self.state_stack.push(BuilderState3::Otherwise);
            }
            b"xsl:for-each" | b"xsl:apply-templates" | b"xsl:for-each-group" => {
                self.state_stack.push(BuilderState3::Sortable {
                    attrs,
                    sort_keys: Vec::new(),
                    saw_non_sort_child: false,
                });
            }
            b"xsl:call-template" => {
                self.handle_call_template_start(attrs, pos, source)?;
            }
            b"xsl:next-iteration" => {
                self.handle_next_iteration_start()?;
            }
            b"xsl:copy" | b"xsl:copy-of" => {
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
            b"xsl:message" => {
                self.handle_message_start(&attrs)?;
            }
            b"xsl:assert" => {
                self.handle_assert_start(attrs, pos, source)?;
            }
            b"xsl:on-empty" | b"xsl:on-non-empty" | b"xsl:where-populated" => {
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
            b"xsl:analyze-string" => {
                self.handle_analyze_string_start(attrs, pos, source)?;
            }
            b"xsl:matching-substring" => {
                self.state_stack.push(BuilderState3::MatchingSubstring);
            }
            b"xsl:non-matching-substring" => {
                self.state_stack.push(BuilderState3::NonMatchingSubstring);
            }
            b"xsl:perform-sort" => {
                self.handle_perform_sort_start(&attrs)?;
            }
            b"xsl:variable" => {
                self.handle_variable_start(attrs, pos, source)?;
            }
            b"xsl:fallback" => {
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
            _ => {
                self.state_stack.push(BuilderState3::InstructionBody(attrs));
            }
        }

        Ok(())
    }

    fn empty_element(
        &mut self,
        e: &BytesStart,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name_binding = e.name();
        let name = name_binding.as_ref();

        if name != b"xsl:sort"
            && name != b"xsl:merge-key"
            && let Some(BuilderState3::Sortable {
                saw_non_sort_child, ..
            }) = self.state_stack.last_mut()
        {
            *saw_non_sort_child = true;
        }

        match name {
            b"xsl:import" => {
                self.handle_import(&attrs, pos, source)?;
            }
            b"xsl:include" => {
                self.handle_include(&attrs, pos, source)?;
            }
            b"xsl:param" => {
                self.handle_param(&attrs, pos, source)?;
            }
            b"xsl:with-param" => {
                self.handle_with_param(&attrs, pos, source)?;
            }
            b"xsl:variable" => {
                self.handle_variable(&attrs, pos, source)?;
            }
            b"xsl:value-of" => {
                self.handle_value_of(&attrs, pos, source)?;
            }
            b"xsl:copy-of" => {
                self.handle_copy_of(&attrs, pos, source)?;
            }
            b"xsl:sequence" => {
                self.handle_sequence(&attrs, pos, source)?;
            }
            b"xsl:number" => {
                self.handle_number(&attrs)?;
            }
            b"xsl:sort" => {
                self.handle_sort(&attrs, pos, source)?;
            }
            b"xsl:apply-templates" => {
                self.handle_apply_templates_empty(&attrs)?;
            }
            b"xsl:next-iteration" => {
                self.handle_next_iteration(&attrs, pos, source)?;
            }
            b"xsl:break" => {
                self.handle_break(&attrs, pos, source)?;
            }
            b"xsl:output" => {
                self.handle_output(&attrs)?;
            }
            b"xsl:decimal-format" => {
                self.handle_decimal_format(&attrs)?;
            }
            b"xsl:namespace-alias" => {
                self.handle_namespace_alias(&attrs, pos, source)?;
            }
            b"xsl:use-package" => {
                self.handle_use_package(&attrs, pos, source)?;
            }
            b"xsl:key" => {
                self.handle_key(&attrs, pos, source)?;
            }
            b"xsl:attribute-set" => {
                self.handle_attribute_set_empty(&attrs, pos, source)?;
            }
            b"xsl:output-character" => {
                self.handle_output_character(&attrs, pos, source)?;
            }
            b"xsl:preserve-space" => {
                self.handle_preserve_space(&attrs, pos, source)?;
            }
            b"xsl:strip-space" => {
                self.handle_strip_space(&attrs, pos, source)?;
            }
            b"xsl:attribute" => {
                self.handle_attribute_empty(&attrs, pos, source)?;
            }
            b"xsl:accumulator-before" | b"xsl:accumulator-after" => {
                self.handle_accumulator_ref(name, &attrs, pos, source)?;
            }
            b"fo:simple-page-master" => {
                self.handle_simple_page_master(&attrs)?;
            }
            b"xsl:json-to-xml" => {
                self.handle_json_to_xml(&attrs, pos, source)?;
            }
            b"xsl:xml-to-json" => {
                self.handle_xml_to_json(&attrs)?;
            }
            b"xsl:evaluate" => {
                self.handle_evaluate(&attrs, pos, source)?;
            }
            b"xsl:next-match" => {
                self.handle_next_match(&attrs, pos, source)?;
            }
            b"xsl:apply-imports" => {
                self.handle_apply_imports(&attrs)?;
            }
            b"xsl:mode" => {
                self.handle_mode(&attrs)?;
            }
            b"xsl:context-item" => {
                self.handle_context_item(&attrs)?;
            }
            b"xsl:global-context-item" => {
                self.handle_global_context_item(&attrs)?;
            }
            b"xsl:initial-template" => {
                self.handle_initial_template(&attrs, pos, source)?;
            }
            b"xsl:stream" => {
                self.features.uses_streaming = true;
                self.handle_stream_empty(&attrs, pos, source)?;
            }
            b"xsl:source-document" => {
                self.handle_source_document_empty(&attrs, pos, source)?;
            }
            b"xsl:map-entry" => {
                self.handle_map_entry_empty(&attrs, pos, source)?;
            }
            b"xsl:array-member" => {
                self.handle_array_member_empty(&attrs, pos, source)?;
            }
            _ => {
                let styles = self.resolve_styles(&attrs)?;
                let non_style_attrs = self.get_non_style_attributes(&attrs)?;
                let shadow_attrs = self.get_shadow_attributes(&attrs)?;
                let use_attribute_sets = self.get_use_attribute_sets(&attrs)?;
                let tag_name = self.apply_namespace_aliases(e.name().as_ref());
                let instr = Xslt3Instruction::EmptyTag {
                    tag_name,
                    styles,
                    attrs: non_style_attrs,
                    shadow_attrs,
                    use_attribute_sets,
                };
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.push(instr);
                }
            }
        }

        Ok(())
    }

    fn end_element(&mut self, e: &BytesEnd, pos: usize, source: &str) -> Result<(), Xslt3Error> {
        let name_binding = e.name();
        let name = name_binding.as_ref();
        self.pop_expand_text();
        let body = self.instruction_stack.pop().unwrap_or_default();
        let current_state = self.state_stack.pop().unwrap_or(BuilderState3::Stylesheet);

        match name {
            b"xsl:stylesheet" | b"xsl:transform" | b"xsl:package" => {}
            b"xsl:template" => {
                self.handle_template_end(current_state, body, pos, source)?;
            }
            b"xsl:function" => {
                self.handle_function_end(current_state, body)?;
            }
            b"xsl:accumulator" => {
                self.handle_accumulator_end(current_state)?;
            }
            b"xsl:accumulator-rule" => {
                self.handle_accumulator_rule_end(current_state, body, pos, source)?;
            }
            b"xsl:attribute-set" => {
                self.handle_attribute_set_end(current_state, body)?;
            }
            b"xsl:character-map" => {
                self.handle_character_map_end(current_state)?;
            }
            b"xsl:try" => {
                self.handle_try_end(current_state, body)?;
            }
            b"xsl:catch" => {
                self.handle_catch_end(current_state, body)?;
            }
            b"xsl:iterate" => {
                self.handle_iterate_end(current_state, body)?;
            }
            b"xsl:on-completion" => {
                self.handle_on_completion_end(body)?;
            }
            b"xsl:map" => {
                self.handle_map_end(current_state)?;
            }
            b"xsl:map-entry" => {
                self.handle_map_entry_end(current_state, body)?;
            }
            b"xsl:array" => {
                self.handle_array_end(current_state)?;
            }
            b"xsl:array-member" => {
                self.handle_array_member_end(body)?;
            }
            b"xsl:fork" => {
                self.handle_fork_end(current_state)?;
            }
            b"xsl:sequence" => {
                if let BuilderState3::InstructionBody(attrs) = current_state {
                    if let Some(BuilderState3::Fork { branches }) = self.state_stack.last_mut() {
                        branches.push(crate::ast::ForkBranch {
                            body: PreparsedTemplate(body),
                        });
                    } else if let Some(select_str) = get_attr_optional(&attrs, b"select")? {
                        let expr = self.parse_xpath(&select_str)?;
                        let instr = Xslt3Instruction::Sequence { select: expr };
                        if let Some(parent) = self.instruction_stack.last_mut() {
                            parent.push(instr);
                        }
                    } else if !body.is_empty()
                        && let Some(parent) = self.instruction_stack.last_mut()
                    {
                        parent.extend(body);
                    }
                }
            }
            b"xsl:merge" => {
                self.handle_merge_end(current_state)?;
            }
            b"xsl:merge-source" => {
                self.handle_merge_source_end(current_state)?;
            }
            b"xsl:merge-action" => {
                self.handle_merge_action_end(body)?;
            }
            b"xsl:source-document" => {
                self.handle_source_document_end(current_state, body)?;
            }
            b"xsl:result-document" => {
                self.handle_result_document_end(current_state, body)?;
            }
            b"xsl:stream" => {
                self.handle_stream_end(current_state, body)?;
            }
            b"xsl:text" => {
                self.handle_text_end(body)?;
            }
            b"xsl:if" => {
                self.handle_if_end(current_state, body, pos, source)?;
            }
            b"xsl:choose" => {
                self.handle_choose_end(current_state)?;
            }
            b"xsl:when" => {
                self.handle_when_end(current_state, body, pos, source)?;
            }
            b"xsl:otherwise" => {
                self.handle_otherwise_end(body)?;
            }
            b"xsl:for-each" => {
                self.handle_for_each_end(current_state, body, pos, source)?;
            }
            b"xsl:for-each-group" => {
                self.handle_for_each_group_end(current_state, body, pos, source)?;
            }
            b"xsl:apply-templates" => {
                self.handle_apply_templates_end(current_state, pos, source)?;
            }
            b"xsl:call-template" => {
                self.handle_call_template_end(current_state)?;
            }
            b"xsl:next-iteration" => {
                self.handle_next_iteration_end(current_state)?;
            }
            b"xsl:copy" => {
                self.handle_copy_end(current_state, body)?;
            }
            b"xsl:message" => {
                self.handle_message_end(current_state, body)?;
            }
            b"xsl:assert" => {
                self.handle_assert_end(current_state, body)?;
            }
            b"xsl:on-empty" => {
                let instr = Xslt3Instruction::OnEmpty {
                    body: PreparsedTemplate(body),
                };
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.push(instr);
                }
            }
            b"xsl:on-non-empty" => {
                let instr = Xslt3Instruction::OnNonEmpty {
                    body: PreparsedTemplate(body),
                };
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.push(instr);
                }
            }
            b"xsl:where-populated" => {
                let instr = Xslt3Instruction::WherePopulated {
                    body: PreparsedTemplate(body),
                };
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.push(instr);
                }
            }
            b"xsl:analyze-string" => {
                self.handle_analyze_string_end(current_state)?;
            }
            b"xsl:matching-substring" => {
                self.handle_matching_substring_end(body)?;
            }
            b"xsl:non-matching-substring" => {
                self.handle_non_matching_substring_end(body)?;
            }
            b"xsl:perform-sort" => {
                self.handle_perform_sort_end(current_state, body)?;
            }
            b"xsl:variable" => {
                self.handle_variable_end(current_state, body)?;
            }
            b"xsl:fallback" => {
                self.handle_fallback_end(body)?;
            }
            _ => {
                self.handle_literal_element_end(e, current_state, body)?;
            }
        }

        Ok(())
    }

    fn text(&mut self, text: String) -> Result<(), Xslt3Error> {
        if let Some(BuilderState3::Sortable {
            saw_non_sort_child, ..
        }) = self.state_stack.last_mut()
            && !text.trim().is_empty()
        {
            *saw_non_sort_child = true;
        }

        let is_in_xsl_text = matches!(self.state_stack.last(), Some(BuilderState3::XslText));
        let should_expand = self.current_expand_text() && !is_in_xsl_text;

        if !is_in_xsl_text && text.trim().is_empty() {
            return Ok(());
        }

        let instr = if should_expand {
            let tvt = self.parse_tvt(&text)?;
            if tvt.0.len() == 1
                && let Some(TvtPart::Static(s)) = tvt.0.first()
            {
                Xslt3Instruction::Text(s.clone())
            } else if !tvt.0.is_empty() {
                Xslt3Instruction::TextValueTemplate(tvt)
            } else {
                return Ok(());
            }
        } else {
            Xslt3Instruction::Text(text)
        };

        if let Some(body) = self.instruction_stack.last_mut() {
            body.push(instr);
        }

        Ok(())
    }
}

pub fn get_attr_optional(
    attrs: &OwnedAttributes,
    name: &[u8],
) -> Result<Option<String>, Xslt3Error> {
    for (key, value) in attrs {
        if key == name {
            return from_utf8(value)
                .map(|s| Some(s.to_string()))
                .map_err(|e| Xslt3Error::parse(e.to_string()));
        }
    }
    Ok(None)
}

pub fn get_attr_required(
    attrs: &OwnedAttributes,
    name: &[u8],
    element: &[u8],
    pos: usize,
    source: &str,
) -> Result<String, Xslt3Error> {
    get_attr_optional(attrs, name)?.ok_or_else(|| {
        let (line, col) = get_line_col_from_pos(source, pos);
        Xslt3Error::parse(format!(
            "Required attribute '{}' missing on element '{}' at line {}:{}",
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(element),
            line,
            col
        ))
    })
}

fn get_line_col_from_pos(source: &str, pos: usize) -> (usize, usize) {
    let prefix = &source[..pos.min(source.len())];
    let line = prefix.matches('\n').count() + 1;
    let col = prefix.rfind('\n').map_or(pos + 1, |nl| pos - nl);
    (line, col)
}
