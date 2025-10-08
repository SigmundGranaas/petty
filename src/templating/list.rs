// FILE: /home/sigmund/RustroverProjects/petty/src/core/templating/list.rs

use crate::core::style::stylesheet::ElementStyle;
use crate::parser::json::ast::{JsonContainer, JsonNode, TemplateNode};
use crate::templating::builders::ListItem;
use crate::templating::node::TemplateBuilder;

/// Builder for a `<List>` node.
#[derive(Default, Clone)]
pub struct List {
    style_names: Vec<String>,
    style_override: ElementStyle,
    children: Vec<Box<dyn TemplateBuilder>>,
}

impl List {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a list item to the list.
    pub fn item(mut self, item: ListItem) -> Self {
        self.children.push(Box::new(item));
        self
    }

    pub fn style_name(mut self, name: &str) -> Self {
        self.style_names.push(name.to_string());
        self
    }

    pub fn with_override(mut self, style: ElementStyle) -> Self {
        self.style_override = style;
        self
    }
}

impl TemplateBuilder for List {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::List(JsonContainer {
            style_names: self.style_names,
            style_override: self.style_override,
            children: self.children.into_iter().map(|c| c.build()).collect(),
        }))
    }
}