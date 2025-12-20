use crate::node::TemplateBuilder;
use crate::style::impl_styled_widget;
use petty_json_template::ast::{JsonImage, JsonNode, TemplateNode};
use petty_style::stylesheet::ElementStyle;

/// Builder for a block-level `<Image>` node.
#[derive(Clone)]
pub struct Image {
    id: Option<String>,
    src: String,
    style_names: Vec<String>,
    style_override: ElementStyle,
}

impl Image {
    pub fn new(src: &str) -> Self {
        Self {
            id: None,
            src: src.to_string(),
            style_names: vec![],
            style_override: Default::default(),
        }
    }

    pub fn style_name(mut self, name: &str) -> Self {
        self.style_names.push(name.to_string());
        self
    }
}

impl TemplateBuilder for Image {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::Image(JsonImage {
            id: self.id,
            src: self.src,
            style_names: self.style_names,
            style_override: self.style_override,
        }))
    }
}

impl_styled_widget!(Image);
