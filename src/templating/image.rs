// FILE: /home/sigmund/RustroverProjects/petty/src/core/templating/image.rs

use crate::core::style::stylesheet::ElementStyle;
use crate::parser::json::ast::{JsonImage, JsonNode, TemplateNode};
use crate::templating::node::TemplateBuilder;

/// Builder for a block-level `<Image>` node.
#[derive(Clone)]
pub struct Image {
    src: String,
    style_names: Vec<String>,
    style_override: ElementStyle,
}

impl Image {
    pub fn new(src: &str) -> Self {
        Self {
            src: src.to_string(),
            style_names: vec![],
            style_override: Default::default(),
        }
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

impl TemplateBuilder for Image {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::Image(JsonImage {
            src: self.src,
            style_names: self.style_names,
            style_override: self.style_override,
        }))
    }
}