use crate::xpath::{Condition, Selection};
use std::collections::HashMap;
use std::sync::Arc;
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::ElementStyle;

/// Represents a pre-compiled, executable block of XSLT.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparsedTemplate(pub Vec<XsltInstruction>);

/// A struct to hold pre-resolved styles for a single instruction.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreparsedStyles {
    /// A list of pre-resolved, shared pointers to named styles (from attribute-sets).
    pub style_sets: Vec<Arc<ElementStyle>>,
    /// An optional inline style override (from FO attributes like font-size="...").
    pub style_override: Option<ElementStyle>,
}

/// The definition of a table column, parsed at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumnDefinition {
    pub width: Option<Dimension>,
    pub style: Option<String>,
    pub header_style: Option<String>,
}

/// Represents a parameter passed to a template.
#[derive(Debug, Clone, PartialEq)]
pub struct WithParam {
    pub name: String,
    pub select: Selection,
}

/// Represents a single compiled `<xsl:template match="...">` rule.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRule {
    /// The XPath match pattern (e.g., "item", "*", "text()").
    pub match_pattern: String,
    /// The calculated or specified priority of the rule.
    pub priority: f64,
    /// The mode this rule belongs to. `None` represents the default mode.
    pub mode: Option<String>,
    /// The compiled body of the template.
    pub body: PreparsedTemplate,
}

/// The complete output of the XSLT compiler.
#[derive(Debug, Clone, Default)]
pub struct CompiledStylesheet {
    /// The template for the root node (`match="/"`).
    pub root_template: Option<PreparsedTemplate>,
    /// All match-based template rules, grouped by mode.
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule>>,
    /// All named templates, for use with `<xsl:call-template>`.
    pub named_templates: HashMap<String, PreparsedTemplate>,
    /// All named attribute sets from `<xsl:attribute-set>`.
    pub styles: HashMap<String, Arc<ElementStyle>>,
    // TODO: Add user-defined functions (`<xsl:function>`).
    // pub functions: HashMap<String, CompiledFunction>,
}

/// An instruction in a pre-parsed template, representing a node or control flow statement.
#[derive(Debug, Clone, PartialEq)]
pub enum XsltInstruction {
    /// A literal block of text, potentially with Handlebars templates.
    Text(String),
    /// A standard content tag like `<container>` or `<text>`.
    ContentTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, String>,
        body: PreparsedTemplate,
    },
    /// A self-closing tag like `<br/>` or `<image>`.
    EmptyTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, String>,
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
    /// A `call-template` instruction with a compiled name and parameters.
    CallTemplate {
        name: String,
        params: Vec<WithParam>,
    },
    /// An `apply-templates` instruction, the core of the push model.
    ApplyTemplates {
        select: Option<Selection>,
        mode: Option<String>,
    },
    /// A structured `table` block, with pre-parsed components.
    Table {
        styles: PreparsedStyles,
        columns: Vec<TableColumnDefinition>,
        header: Option<PreparsedTemplate>,
        body: PreparsedTemplate,
    },
}