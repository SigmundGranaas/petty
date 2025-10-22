// FILE: src/parser/xslt/ast.rs
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::parser::xpath::Expression;
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

#[derive(Debug, Clone)]
pub struct CompiledStylesheet {
    pub stylesheet: Stylesheet,
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule>>,
    pub named_templates: HashMap<String, Arc<NamedTemplate>>,
    pub resource_base_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XsltInstruction {
    Text(String),
    ContentTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, String>,
        body: PreparsedTemplate,
    },
    EmptyTag {
        tag_name: Vec<u8>,
        styles: PreparsedStyles,
        attrs: HashMap<String, String>,
    },
    If {
        test: Expression,
        body: PreparsedTemplate,
    },
    ForEach {
        select: Expression,
        body: PreparsedTemplate,
    },
    ValueOf {
        select: Expression,
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
        mode: Option<String>,
    },
    Table {
        styles: PreparsedStyles,
        columns: Vec<Dimension>,
        header: Option<PreparsedTemplate>,
        body: PreparsedTemplate,
    },
    PageBreak {
        master_name: Option<String>,
    },
}