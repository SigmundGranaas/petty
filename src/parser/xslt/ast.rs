use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, PageLayout};
use crate::xpath::{Condition, Selection};
use std::collections::HashMap;
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
pub struct TableColumnDefinition {
    pub width: Option<Dimension>,
    pub style: Option<String>,
    pub header_style: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithParam {
    pub name: String,
    pub select: Selection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRule {
    pub match_pattern: String,
    pub priority: f64,
    pub mode: Option<String>,
    pub body: PreparsedTemplate,
}

#[derive(Debug, Clone, Default)]
pub struct CompiledStylesheet {
    pub page: PageLayout,
    pub root_template: Option<PreparsedTemplate>,
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule>>,
    pub named_templates: HashMap<String, PreparsedTemplate>,
    pub styles: HashMap<String, Arc<ElementStyle>>,
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
        test: Condition,
        body: PreparsedTemplate,
    },
    ForEach {
        select: Selection,
        body: PreparsedTemplate,
    },
    ValueOf {
        select: Selection,
    },
    CallTemplate {
        name: String,
        params: Vec<WithParam>,
    },
    ApplyTemplates {
        select: Option<Selection>,
        mode: Option<String>,
    },
    Table {
        styles: PreparsedStyles,
        columns: Vec<TableColumnDefinition>,
        header: Option<PreparsedTemplate>,
        body: PreparsedTemplate,
    },
}