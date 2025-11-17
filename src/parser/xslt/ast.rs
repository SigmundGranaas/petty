// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/ast.rs
// FILE: src/parser/xslt/ast.rs
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::parser::processor::TemplateFlags;
use crate::parser::xslt::xpath::Expression;
use crate::parser::xslt::pattern::Pattern;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Represents a pre-compiled, executable block of XSLT.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparsedTemplate(pub Vec<XsltInstruction>);

/// A struct to hold pre-resolved styles for a single instruction.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreparsedStyles {
    pub id: Option<String>,
    pub style_sets: Vec<Arc<ElementStyle>>,
    pub style_override: Option<ElementStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithParam {
    pub name: String,
    pub select: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRule {
    pub pattern: Pattern, // Changed from String to compiled Pattern
    pub priority: f64,
    pub mode: Option<String>,
    pub body: PreparsedTemplate,
}

/// Represents a declared `<xsl:param>` in a template.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub default_value: Option<Expression>,
}

/// Represents a compiled, named `<xsl:template name="...">`.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedTemplate {
    pub params: Vec<Param>,
    pub body: PreparsedTemplate,
}

/// Represents a compiled `<xsl:key>` definition.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyDefinition {
    pub name: String,
    pub pattern: Pattern,
    pub use_expr: Expression,
}

#[derive(Debug, Clone)]
pub struct CompiledStylesheet {
    pub stylesheet: Arc<Stylesheet>,
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule>>,
    pub named_templates: HashMap<String, Arc<NamedTemplate>>,
    pub keys: Vec<KeyDefinition>,
    pub resource_base_path: PathBuf,
    /// Maps a role name (e.g., "page-header") to a unique, generated mode name.
    pub role_template_modes: HashMap<String, String>,
    /// Flags for features detected across the entire stylesheet.
    pub features: TemplateFlags,
}

/// Represents a compiled `<xsl:when>` block.
#[derive(Debug, Clone, PartialEq)]
pub struct When {
    pub test: Expression,
    pub body: PreparsedTemplate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDataType {
    Text,
    Number,
}

/// Represents a compiled `<xsl:sort>` instruction.
#[derive(Debug, Clone, PartialEq)]
pub struct SortKey {
    pub select: Expression,
    pub order: SortOrder,
    pub data_type: SortDataType,
}

/// A part of an Attribute Value Template.
#[derive(Debug, Clone, PartialEq)]
pub enum AvtPart {
    Static(String),
    Dynamic(Expression),
}

/// A pre-compiled attribute value that can contain XPath expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValueTemplate {
    Static(String),
    Dynamic(Vec<AvtPart>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum XsltInstruction {
    Text(String),
    ContentTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, AttributeValueTemplate>,
        body: PreparsedTemplate,
    },
    EmptyTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, AttributeValueTemplate>,
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
        whens: Vec<When>,
        otherwise: Option<PreparsedTemplate>,
    },
    ForEach {
        select: Expression,
        sort_keys: Vec<SortKey>,
        body: PreparsedTemplate,
    },
    ValueOf {
        select: Expression,
    },
    CopyOf {
        select: Expression,
    },
    Copy {
        styles: PreparsedStyles,
        body: PreparsedTemplate,
    },
    Variable {
        name: String,
        select: Expression,
    },
    CallTemplate {
        name: String,
        params: Vec<WithParam>,
    },
    ApplyTemplates {
        select: Option<Expression>,
        mode: Option<AttributeValueTemplate>,
        sort_keys: Vec<SortKey>,
    },
    Table {
        styles: PreparsedStyles,
        columns: Vec<Dimension>,
        header: Option<PreparsedTemplate>,
        body: PreparsedTemplate,
    },
    TableOfContents {
        styles: PreparsedStyles,
    },
    PageBreak {
        master_name: Option<AttributeValueTemplate>,
    },
}