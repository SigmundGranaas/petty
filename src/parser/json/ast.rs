// src/parser/json/ast.rs
// src/parser/json/ast.rs
//! Defines the Abstract Syntax Tree (AST) for the JSON template format as it is
//! parsed from the source file by Serde. This is the **input** representation.

use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, PageLayout};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// --- Template Structure ---

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum TemplateNode {
    Control(ControlNode),
    Static(JsonNode),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum ControlNode {
    /// Iterates over an array in the data context.
    Each {
        /// A JPath expression resolving to an array.
        each: String,
        /// The template to instantiate for each item in the array.
        template: Box<TemplateNode>,
    },
    /// Conditionally renders a template.
    If {
        /// A JPath expression that evaluates to a truthy value.
        #[serde(rename = "if")]
        test: String,
        /// The template to render if the condition is true.
        then: Box<TemplateNode>,
        /// The optional template to render if the condition is false.
        #[serde(rename = "else")]
        #[serde(default)]
        else_branch: Option<Box<TemplateNode>>,
    },
}

// --- Combined and Tagged JsonNode ---

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "PascalCase")]
pub enum JsonNode {
    // Block-level variants
    Block(JsonContainer),
    FlexContainer(JsonContainer),
    Paragraph(JsonParagraph),
    Image(JsonImage),
    List(JsonContainer),
    ListItem(JsonContainer),
    Table(JsonTable),
    Heading(JsonHeading),
    TableOfContents(JsonContainer),
    IndexMarker {
        term: String,
    },
    // Inline-level variants
    Text {
        content: String,
    },
    StyledSpan(JsonInlineContainer),
    Hyperlink(JsonHyperlink),
    PageReference {
        #[serde(rename = "targetId")]
        target_id: String,
    },
    InlineImage(JsonImage),
    LineBreak,
    // New control-flow nodes
    PageBreak {
        #[serde(rename = "masterName", skip_serializing_if = "Option::is_none")]
        master_name: Option<String>,
    },
    RenderTemplate {
        name: String,
    },
}

// --- Component Structs ---

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonContainer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub style_names: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub style_override: ElementStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonParagraph {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub style_names: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub style_override: ElementStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonImage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub src: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub style_names: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub style_override: ElementStyle,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonInlineContainer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub style_names: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub style_override: ElementStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonHyperlink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub href: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub style_names: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub style_override: ElementStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonTable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub style_names: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub style_override: ElementStyle,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<JsonTableColumn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<JsonTableHeader>,
    pub body: JsonTableBody,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonTableColumn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<Dimension>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_style: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonTableHeader {
    pub rows: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct JsonTableBody {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonHeading {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub style_names: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub style_override: ElementStyle,
    #[serde(default = "default_heading_level")]
    pub level: u8,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TemplateNode>,
}

fn default_heading_level() -> u8 {
    1
}
// --- Top-level Template and Stylesheet ---

#[derive(Deserialize, Serialize, Debug)]
pub struct JsonTemplateFile {
    pub _stylesheet: StylesheetDef,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub _roles: HashMap<String, TemplateNode>,
    pub _template: TemplateNode,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StylesheetDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_page_master: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub page_masters: HashMap<String, PageLayout>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub styles: HashMap<String, ElementStyle>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub definitions: HashMap<String, TemplateNode>,
}

/// Helper for serde to skip serializing default empty values for cleaner JSON.
fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    *t == T::default()
}