use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, Stylesheet};
use crate::xpath::{Condition, Selection};
use handlebars::Handlebars;
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
    pub select: Selection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRule {
    pub match_pattern: String,
    pub priority: f64,
    pub mode: Option<String>,
    pub body: PreparsedTemplate,
}

#[derive(Debug, Clone)]
pub struct CompiledStylesheet {
    pub stylesheet: Stylesheet,
    pub root_template: Option<PreparsedTemplate>,
    pub template_rules: HashMap<Option<String>, Vec<TemplateRule>>,
    pub named_templates: HashMap<String, PreparsedTemplate>,
    pub resource_base_path: PathBuf,
    // Handlebars is needed at execution time for {{...}} expressions
    pub handlebars: Handlebars<'static>,
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
        columns: Vec<Dimension>, // Simplified for now
        header: Option<PreparsedTemplate>,
        body: PreparsedTemplate,
    },
    PageBreak {
        master_name: Option<String>,
    },
}