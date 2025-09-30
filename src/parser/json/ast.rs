//! Defines the Abstract Syntax Tree (AST) for the JSON template format as it is
//! parsed from the source file by Serde. This is the **input** representation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::{ElementStyle, PageLayout};
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
        /// A JSON pointer or Handlebars expression resolving to an array.
        each: String,
        /// The template to instantiate for each item in the array.
        template: Box<TemplateNode>,
    },
    /// Conditionally renders a template.
    If {
        /// A Handlebars expression that evaluates to a truthy value.
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
    // Inline-level variants
    Text {
        content: String,
    },
    StyledSpan(JsonInlineContainer),
    Hyperlink(JsonHyperlink),
    InlineImage(JsonImage),
    LineBreak,
}

// --- Component Structs ---

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonContainer {
    #[serde(default)]
    pub style_names: Vec<String>,
    #[serde(default)]
    pub style_override: ElementStyle,
    #[serde(default)]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonParagraph {
    #[serde(default)]
    pub style_names: Vec<String>,
    #[serde(default)]
    pub style_override: ElementStyle,
    #[serde(default)]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonImage {
    pub src: String,
    #[serde(default)]
    pub style_names: Vec<String>,
    #[serde(default)]
    pub style_override: ElementStyle,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonInlineContainer {
    #[serde(default)]
    pub style_names: Vec<String>,
    #[serde(default)]
    pub style_override: ElementStyle,
    #[serde(default)]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonHyperlink {
    pub href: String,
    #[serde(default)]
    pub style_names: Vec<String>,
    #[serde(default)]
    pub style_override: ElementStyle,
    #[serde(default)]
    pub children: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonTable {
    #[serde(default)]
    pub style_names: Vec<String>,
    #[serde(default)]
    pub style_override: ElementStyle,
    #[serde(default)]
    pub columns: Vec<JsonTableColumn>,
    pub header: Option<JsonTableHeader>,
    pub body: JsonTableBody,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JsonTableColumn {
    pub width: Option<Dimension>,
    pub style: Option<String>,
    pub header_style: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonTableHeader {
    pub rows: Vec<TemplateNode>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct JsonTableBody {
    #[serde(default)]
    pub rows: Vec<TemplateNode>,
}

// --- Top-level Template and Stylesheet ---

#[derive(Deserialize, Debug)]
pub struct JsonTemplateFile {
    pub _stylesheet: StylesheetDef,
    pub _template: TemplateNode,
}

#[derive(Deserialize, Debug, Default)]
pub struct StylesheetDef {
    #[serde(default)]
    pub page: PageLayout,
    #[serde(default)]
    pub styles: HashMap<String, ElementStyle>,
}