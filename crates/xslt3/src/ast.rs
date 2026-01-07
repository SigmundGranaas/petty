//! XSLT 3.0 abstract syntax tree types.
//!
//! This module defines the data structures that represent a compiled XSLT 3.0 stylesheet.
//! The compiler transforms XSLT source into these AST types, which are then interpreted
//! by the executor to produce output.
//!
//! # Core Types
//!
//! | Type | Description |
//! |------|-------------|
//! | [`CompiledStylesheet3`] | Root container holding all stylesheet declarations |
//! | [`Xslt3Instruction`] | Enum with ~50 variants for all XSLT 3.0 instructions |
//! | [`TemplateRule3`] | Match template with pattern, priority, and mode |
//! | [`NamedTemplate3`] | Named template callable via `xsl:call-template` |
//! | [`Function3`] | Stylesheet function callable from XPath expressions |
//! | [`Accumulator`] | Streaming accumulator with rules for stateful processing |
//!
//! # Instruction Categories
//!
//! The [`Xslt3Instruction`] enum covers:
//! - **Output**: `Text`, `ValueOf`, `CopyOf`, `Sequence`, literal result elements
//! - **Control Flow**: `If`, `Choose`, `ForEach`, `ForEachGroup`, `Iterate`
//! - **Templates**: `ApplyTemplates`, `CallTemplate`, `NextMatch`
//! - **Variables**: `Variable`, `Param`
//! - **XSLT 3.0**: `Try`/`Catch`, `Map`, `Array`, `Fork`, `Merge`, `AnalyzeString`
//!
//! # Expression Types
//!
//! - [`Expression`]: XPath 3.1 expression from `petty_xpath31`
//! - [`Avt3`]: Attribute Value Template (static or dynamic)
//! - [`TextValueTemplate`]: Text content with embedded expressions

use petty_xpath31::Expression;
use std::collections::HashMap;
use std::sync::Arc;

pub use petty_xslt::ast::{
    AttributeValueTemplate, AvtPart, Param, PreparsedStyles, SortDataType, SortKey, SortOrder,
    When, WithParam,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Avt3 {
    Static(String),
    Dynamic(Vec<Avt3Part>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Avt3Part {
    Static(String),
    Dynamic(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparsedTemplate(pub Vec<Xslt3Instruction>);

#[derive(Debug, Clone, PartialEq)]
pub struct Pattern3(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRule3 {
    pub pattern: Pattern3,
    pub priority: f64,
    pub mode: Option<String>,
    pub body: PreparsedTemplate,
    pub visibility: Visibility,
    pub from_import: bool,
    pub import_precedence: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedTemplate3 {
    pub params: Vec<Param3>,
    pub body: PreparsedTemplate,
    pub visibility: Visibility,
    pub as_type: Option<SequenceType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param3 {
    pub name: String,
    pub default_value: Option<Expression>,
    pub as_type: Option<SequenceType>,
    pub required: bool,
    pub tunnel: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    Public,
    #[default]
    Private,
    Final,
    Abstract,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceType {
    pub item_type: String,
    pub occurrence: OccurrenceIndicator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccurrenceIndicator {
    ExactlyOne,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function3 {
    pub name: String,
    pub params: Vec<Param3>,
    pub body: PreparsedTemplate,
    pub as_type: Option<SequenceType>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Accumulator {
    pub name: String,
    pub initial_value: Expression,
    pub rules: Vec<AccumulatorRule>,
    pub streamable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccumulatorRule {
    pub pattern: Pattern3,
    pub phase: AccumulatorPhase,
    pub select: Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccumulatorPhase {
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeSource {
    pub name: Option<String>,
    pub for_each_item: Option<Expression>,
    pub for_each_source: Option<AttributeValueTemplate>,
    pub select: Expression,
    pub sort_keys: Vec<MergeKey>,
    pub streamable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeKey {
    pub select: Expression,
    pub order: SortOrder,
    pub collation: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeAction {
    pub body: PreparsedTemplate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub errors: Vec<String>,
    pub body: PreparsedTemplate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IterateParam {
    pub name: String,
    pub select: Expression,
    pub as_type: Option<SequenceType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NextIterationParam {
    pub name: String,
    pub select: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextValueTemplate(pub Vec<TvtPart>);

#[derive(Debug, Clone, PartialEq)]
pub enum TvtPart {
    Static(String),
    Dynamic(Expression),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ContextItemDeclaration {
    pub as_type: Option<SequenceType>,
    pub use_when: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GlobalContextItemDeclaration {
    pub as_type: Option<SequenceType>,
    pub use_when: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitialTemplateDeclaration {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CompiledStylesheet3 {
    pub version: String,
    pub default_mode: Option<String>,
    pub expand_text: bool,
    pub use_packages: Vec<UsePackage>,
    pub imports: Vec<ImportDeclaration>,
    pub includes: Vec<IncludeDeclaration>,
    pub global_variables: HashMap<String, GlobalVariable>,
    pub global_params: HashMap<String, GlobalParam>,
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule3>>,
    pub named_templates: HashMap<String, Arc<NamedTemplate3>>,
    pub functions: HashMap<String, Arc<Function3>>,
    pub accumulators: HashMap<String, Accumulator>,
    pub keys: HashMap<String, KeyDeclaration>,
    pub attribute_sets: HashMap<String, AttributeSet>,
    pub output: OutputDeclaration,
    pub outputs: HashMap<String, OutputDeclaration>,
    pub decimal_formats: HashMap<Option<String>, DecimalFormatDeclaration>,
    pub namespace_aliases: Vec<NamespaceAlias>,
    pub character_maps: HashMap<String, CharacterMap>,
    pub features: Xslt3Features,
    pub stylesheet: petty_style::stylesheet::Stylesheet,
    pub modes: HashMap<Option<String>, ModeDeclaration>,
    pub context_item: Option<ContextItemDeclaration>,
    pub global_context_item: Option<GlobalContextItemDeclaration>,
    pub initial_template: Option<InitialTemplateDeclaration>,
    pub preserve_space: Vec<String>,
    pub strip_space: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsePackage {
    pub name: String,
    pub version: Option<String>,
    pub overrides: Vec<Override>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportDeclaration {
    pub href: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IncludeDeclaration {
    pub href: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyDeclaration {
    pub name: String,
    pub match_pattern: String,
    pub use_expr: Expression,
    pub collation: Option<String>,
    pub composite: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeSet {
    pub name: String,
    pub use_attribute_sets: Vec<String>,
    pub attributes: PreparsedTemplate,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Override {
    Template {
        name: String,
        body: PreparsedTemplate,
    },
    Function {
        name: String,
        body: PreparsedTemplate,
    },
    Variable {
        name: String,
        select: Expression,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalVariable {
    pub name: String,
    pub select: Expression,
    pub as_type: Option<SequenceType>,
    pub visibility: Visibility,
    pub static_var: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalParam {
    pub name: String,
    pub default_value: Option<Expression>,
    pub as_type: Option<SequenceType>,
    pub required: bool,
    pub static_param: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OutputDeclaration {
    pub name: Option<String>,
    pub method: Option<String>,
    pub version: Option<String>,
    pub encoding: Option<String>,
    pub indent: Option<bool>,
    pub omit_xml_declaration: Option<bool>,
    pub standalone: Option<String>,
    pub doctype_public: Option<String>,
    pub doctype_system: Option<String>,
    pub cdata_section_elements: Vec<String>,
    pub html_version: Option<String>,
    pub include_content_type: Option<bool>,
    pub media_type: Option<String>,
    pub normalization_form: Option<String>,
    pub suppress_indentation: Vec<String>,
    pub undeclare_prefixes: Option<bool>,
    pub use_character_maps: Vec<String>,
    pub byte_order_mark: Option<bool>,
    pub escape_uri_attributes: Option<bool>,
    pub build_tree: Option<bool>,
    pub item_separator: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecimalFormatDeclaration {
    pub name: Option<String>,
    pub decimal_separator: char,
    pub grouping_separator: char,
    pub infinity: String,
    pub minus_sign: char,
    pub nan: String,
    pub percent: char,
    pub per_mille: char,
    pub zero_digit: char,
    pub digit: char,
    pub pattern_separator: char,
    pub exponent_separator: char,
}

impl Default for DecimalFormatDeclaration {
    fn default() -> Self {
        Self {
            name: None,
            decimal_separator: '.',
            grouping_separator: ',',
            infinity: "Infinity".to_string(),
            minus_sign: '-',
            nan: "NaN".to_string(),
            percent: '%',
            per_mille: '\u{2030}',
            zero_digit: '0',
            digit: '#',
            pattern_separator: ';',
            exponent_separator: 'e',
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamespaceAlias {
    pub stylesheet_prefix: String,
    pub result_prefix: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterMap {
    pub name: String,
    pub use_character_maps: Vec<String>,
    pub mappings: Vec<OutputCharacter>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputCharacter {
    pub character: char,
    pub string: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Xslt3Features {
    pub uses_streaming: bool,
    pub uses_higher_order_functions: bool,
    pub uses_maps: bool,
    pub uses_arrays: bool,
    pub uses_try_catch: bool,
    pub uses_iterate: bool,
    pub uses_accumulators: bool,
    pub uses_merge: bool,
    pub uses_fork: bool,
    pub uses_assertions: bool,
    pub package_name: Option<String>,
    pub package_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModeDeclaration {
    pub name: Option<String>,
    pub streamable: bool,
    pub on_no_match: OnNoMatch,
    pub on_multiple_match: OnMultipleMatch,
    pub warning_on_no_match: bool,
    pub warning_on_multiple_match: bool,
    pub visibility: Visibility,
    pub typed: TypedMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnNoMatch {
    #[default]
    DeepSkip,
    ShallowSkip,
    DeepCopy,
    ShallowCopy,
    TextOnlyCopy,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnMultipleMatch {
    #[default]
    UseLast,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypedMode {
    #[default]
    Unspecified,
    Strict,
    Lax,
    Untyped,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShadowAttribute {
    pub name: Avt3,
    pub value: Avt3,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Xslt3Instruction {
    Text(String),
    TextValueTemplate(TextValueTemplate),

    ContentTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, Avt3>,
        shadow_attrs: Vec<ShadowAttribute>,
        use_attribute_sets: Vec<String>,
        body: PreparsedTemplate,
    },
    EmptyTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, Avt3>,
        shadow_attrs: Vec<ShadowAttribute>,
        use_attribute_sets: Vec<String>,
    },
    Attribute {
        name: AttributeValueTemplate,
        body: PreparsedTemplate,
    },
    Element {
        name: AttributeValueTemplate,
        body: PreparsedTemplate,
    },

    If {
        test: Expression,
        body: PreparsedTemplate,
    },
    Choose {
        whens: Vec<When3>,
        otherwise: Option<PreparsedTemplate>,
    },
    ForEach {
        select: Expression,
        sort_keys: Vec<SortKey3>,
        body: PreparsedTemplate,
    },
    ForEachGroup {
        select: Expression,
        group_by: Option<Expression>,
        group_adjacent: Option<Expression>,
        group_starting_with: Option<String>,
        group_ending_with: Option<String>,
        sort_keys: Vec<SortKey3>,
        body: PreparsedTemplate,
    },

    ValueOf {
        select: Expression,
        separator: Option<AttributeValueTemplate>,
    },
    CopyOf {
        select: Expression,
    },
    Copy {
        styles: PreparsedStyles,
        body: PreparsedTemplate,
    },
    Sequence {
        select: Expression,
    },

    Variable {
        name: String,
        select: Option<Expression>,
        body: Option<PreparsedTemplate>,
        as_type: Option<SequenceType>,
    },
    CallTemplate {
        name: String,
        params: Vec<WithParam3>,
    },
    ApplyTemplates {
        select: Option<Expression>,
        mode: Option<AttributeValueTemplate>,
        sort_keys: Vec<SortKey3>,
    },

    Try {
        body: PreparsedTemplate,
        catches: Vec<CatchClause>,
        rollback_output: bool,
    },

    Iterate {
        select: Expression,
        params: Vec<IterateParam>,
        body: PreparsedTemplate,
        on_completion: Option<PreparsedTemplate>,
    },
    NextIteration {
        params: Vec<NextIterationParam>,
    },
    Break {
        select: Option<Expression>,
    },

    Map {
        entries: Vec<MapEntryInstruction>,
    },
    MapEntry {
        key: Expression,
        select: Option<Expression>,
        body: Option<PreparsedTemplate>,
    },
    Array {
        members: Vec<ArrayMemberInstruction>,
    },
    ArrayMember {
        select: Option<Expression>,
        body: Option<PreparsedTemplate>,
    },

    Stream {
        href: AttributeValueTemplate,
        body: PreparsedTemplate,
    },
    SourceDocument {
        href: AttributeValueTemplate,
        streamable: bool,
        body: PreparsedTemplate,
    },
    Fork {
        branches: Vec<ForkBranch>,
    },
    Merge {
        sources: Vec<MergeSource>,
        action: MergeAction,
    },

    Assert {
        test: Expression,
        message: Option<PreparsedTemplate>,
    },
    Message {
        select: Option<Expression>,
        body: Option<PreparsedTemplate>,
        terminate: bool,
        error_code: Option<String>,
    },

    CallFunction {
        name: String,
        args: Vec<Expression>,
    },

    ResultDocument {
        format: Option<String>,
        href: Option<AttributeValueTemplate>,
        body: PreparsedTemplate,
    },

    Comment {
        body: PreparsedTemplate,
    },
    ProcessingInstruction {
        name: AttributeValueTemplate,
        body: PreparsedTemplate,
    },
    Namespace {
        name: AttributeValueTemplate,
        select: Option<Expression>,
    },

    OnEmpty {
        body: PreparsedTemplate,
    },
    OnNonEmpty {
        body: PreparsedTemplate,
    },
    WherePopulated {
        body: PreparsedTemplate,
    },

    AccumulatorBefore {
        name: String,
    },
    AccumulatorAfter {
        name: String,
    },

    JsonToXml {
        select: Expression,
    },
    XmlToJson {
        select: Option<Expression>,
    },
    Evaluate {
        xpath: Expression,
        context_item: Option<Expression>,
        namespace_context: Option<Expression>,
    },

    NextMatch {
        params: Vec<WithParam3>,
    },

    ApplyImports {
        params: Vec<WithParam3>,
    },

    AnalyzeString {
        select: Expression,
        regex: String,
        flags: Option<String>,
        matching_substring: Option<PreparsedTemplate>,
        non_matching_substring: Option<PreparsedTemplate>,
    },

    PerformSort {
        select: Option<Expression>,
        sort_keys: Vec<SortKey3>,
        body: PreparsedTemplate,
    },

    Fallback {
        body: PreparsedTemplate,
    },

    Number {
        level: NumberLevel,
        count: Option<String>,
        from: Option<String>,
        value: Option<Expression>,
        format: Avt3,
        lang: Option<String>,
        letter_value: Option<String>,
        grouping_separator: Option<String>,
        grouping_size: Option<u32>,
        ordinal: Option<String>,
        select: Option<Expression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberLevel {
    #[default]
    Single,
    Multiple,
    Any,
}

#[derive(Debug, Clone, PartialEq)]
pub struct When3 {
    pub test: Expression,
    pub body: PreparsedTemplate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortKey3 {
    pub select: Expression,
    pub order: SortOrder,
    pub data_type: SortDataType,
    pub collation: Option<String>,
    pub stable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithParam3 {
    pub name: String,
    pub select: Expression,
    pub tunnel: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapEntryInstruction {
    pub key: Expression,
    pub select: Option<Expression>,
    pub body: Option<PreparsedTemplate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayMemberInstruction {
    pub select: Option<Expression>,
    pub body: Option<PreparsedTemplate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForkBranch {
    pub body: PreparsedTemplate,
}

impl Default for CompiledStylesheet3 {
    fn default() -> Self {
        Self {
            version: "3.0".to_string(),
            default_mode: None,
            expand_text: false,
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
            stylesheet: petty_style::stylesheet::Stylesheet::default(),
            modes: HashMap::new(),
            context_item: None,
            global_context_item: None,
            initial_template: None,
            preserve_space: Vec::new(),
            strip_space: Vec::new(),
        }
    }
}

impl CompiledStylesheet3 {
    /// Resolve imports/includes per XSLT spec: imports get lower priority, includes get same priority.
    pub fn resolve_imports_includes<F, E>(&mut self, resolver: F) -> Result<(), E>
    where
        F: Fn(&str) -> Result<CompiledStylesheet3, E>,
    {
        let imports = std::mem::take(&mut self.imports);
        for import in imports {
            let imported = resolver(&import.href)?;
            self.merge_imported(imported);
        }

        let includes = std::mem::take(&mut self.includes);
        for include in includes {
            let included = resolver(&include.href)?;
            self.merge_included(included);
        }

        Ok(())
    }

    fn merge_imported(&mut self, imported: CompiledStylesheet3) {
        const IMPORT_PRIORITY_ADJUSTMENT: f64 = -1000.0;

        for (mode, rules) in imported.template_rules {
            let adjusted_rules: Vec<TemplateRule3> = rules
                .into_iter()
                .map(|mut rule| {
                    rule.priority += IMPORT_PRIORITY_ADJUSTMENT;
                    rule.from_import = true;
                    rule
                })
                .collect();

            self.template_rules
                .entry(mode)
                .or_default()
                .extend(adjusted_rules);
        }

        for (name, template) in imported.named_templates {
            self.named_templates.entry(name).or_insert(template);
        }
        for (name, function) in imported.functions {
            self.functions.entry(name).or_insert(function);
        }
        for (name, var) in imported.global_variables {
            self.global_variables.entry(name).or_insert(var);
        }
        for (name, param) in imported.global_params {
            self.global_params.entry(name).or_insert(param);
        }
        for (name, key) in imported.keys {
            self.keys.entry(name).or_insert(key);
        }
        for (name, acc) in imported.accumulators {
            self.accumulators.entry(name).or_insert(acc);
        }
        for (name, attrs) in imported.attribute_sets {
            self.attribute_sets.entry(name).or_insert(attrs);
        }
        for (name, mode) in imported.modes {
            self.modes.entry(name).or_insert(mode);
        }
        for elem in imported.preserve_space {
            if !self.preserve_space.contains(&elem) {
                self.preserve_space.push(elem);
            }
        }
        for elem in imported.strip_space {
            if !self.strip_space.contains(&elem) {
                self.strip_space.push(elem);
            }
        }
        for (name, decl) in imported.decimal_formats {
            self.decimal_formats.entry(name).or_insert(decl);
        }
        for alias in imported.namespace_aliases {
            if !self.namespace_aliases.contains(&alias) {
                self.namespace_aliases.push(alias);
            }
        }
        for (name, map) in imported.character_maps {
            self.character_maps.entry(name).or_insert(map);
        }
        for (name, output) in imported.outputs {
            self.outputs.entry(name).or_insert(output);
        }
    }

    fn merge_included(&mut self, included: CompiledStylesheet3) {
        for (mode, rules) in included.template_rules {
            self.template_rules.entry(mode).or_default().extend(rules);
        }

        for (name, template) in included.named_templates {
            self.named_templates.entry(name).or_insert(template);
        }
        for (name, function) in included.functions {
            self.functions.entry(name).or_insert(function);
        }
        for (name, var) in included.global_variables {
            self.global_variables.entry(name).or_insert(var);
        }
        for (name, param) in included.global_params {
            self.global_params.entry(name).or_insert(param);
        }
        for (name, key) in included.keys {
            self.keys.entry(name).or_insert(key);
        }
        for (name, acc) in included.accumulators {
            self.accumulators.entry(name).or_insert(acc);
        }
        for (name, attrs) in included.attribute_sets {
            self.attribute_sets.entry(name).or_insert(attrs);
        }
        for (name, mode) in included.modes {
            self.modes.entry(name).or_insert(mode);
        }
        for elem in included.preserve_space {
            if !self.preserve_space.contains(&elem) {
                self.preserve_space.push(elem);
            }
        }
        for elem in included.strip_space {
            if !self.strip_space.contains(&elem) {
                self.strip_space.push(elem);
            }
        }
        for (name, decl) in included.decimal_formats {
            self.decimal_formats.entry(name).or_insert(decl);
        }
        for alias in included.namespace_aliases {
            if !self.namespace_aliases.contains(&alias) {
                self.namespace_aliases.push(alias);
            }
        }
        for (name, map) in included.character_maps {
            self.character_maps.entry(name).or_insert(map);
        }
        for (name, output) in included.outputs {
            self.outputs.entry(name).or_insert(output);
        }
    }

    pub fn finalize_after_merge(&mut self) {
        for rules in self.template_rules.values_mut() {
            rules.sort_by(|a, b| {
                b.priority
                    .partial_cmp(&a.priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
}
