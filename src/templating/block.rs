// FILE: /home/sigmund/RustroverProjects/petty/src/templating/block.rs
use crate::core::style::stylesheet::ElementStyle;
use crate::parser::json::ast::{JsonContainer, JsonNode, TemplateNode};
use crate::templating::node::TemplateBuilder;
use crate::templating::style::impl_styled_widget;

macro_rules! define_container_builder {
    ($name:ident, $node_variant:path) => {
        #[derive(Default, Clone)]
        pub struct $name {
            style_names: Vec<String>,
            style_override: ElementStyle,
            children: Vec<Box<dyn TemplateBuilder>>,
        }

        impl $name {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn child(mut self, child: impl TemplateBuilder + 'static) -> Self {
                self.children.push(Box::new(child));
                self
            }

            pub fn style_name(mut self, name: &str) -> Self {
                self.style_names.push(name.to_string());
                self
            }
        }

        impl TemplateBuilder for $name {
            fn build(self: Box<Self>) -> TemplateNode {
                TemplateNode::Static($node_variant(JsonContainer {
                    style_names: self.style_names,
                    style_override: self.style_override,
                    children: self.children.into_iter().map(|c| c.build()).collect(),
                }))
            }
        }
    };
}

define_container_builder!(Block, JsonNode::Block);
define_container_builder!(Flex, JsonNode::FlexContainer);
define_container_builder!(ListItem, JsonNode::ListItem);

impl_styled_widget!(Block, Flex, ListItem);