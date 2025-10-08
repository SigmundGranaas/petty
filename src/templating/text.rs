// FILE: /home/sigmund/RustroverProjects/petty/src/templating/text.rs
use crate::core::style::stylesheet::ElementStyle;
use crate::parser::json::ast::{JsonHyperlink, JsonImage, JsonInlineContainer, JsonNode, TemplateNode};
use crate::templating::node::TemplateBuilder;
use crate::templating::style::impl_styled_widget;

/// Builder for an inline `<Text>` node.
#[derive(Clone)]
pub struct Text {
    content: String,
}

impl Text {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
        }
    }
}

impl TemplateBuilder for Text {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::Text {
            content: self.content,
        })
    }
}

/// Builder for a `<StyledSpan>` node.
#[derive(Default, Clone)]
pub struct Span {
    style_names: Vec<String>,
    style_override: ElementStyle,
    children: Vec<Box<dyn TemplateBuilder>>,
}

impl Span {
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

    pub fn text(self, content: &str) -> Self {
        self.child(Text::new(content))
    }
}

impl TemplateBuilder for Span {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::StyledSpan(JsonInlineContainer {
            style_names: self.style_names,
            style_override: self.style_override,
            children: self.children.into_iter().map(|c| c.build()).collect(),
        }))
    }
}

/// Builder for a `<Hyperlink>` node.
#[derive(Clone)]
pub struct Hyperlink {
    href: String,
    style_names: Vec<String>,
    style_override: ElementStyle,
    children: Vec<Box<dyn TemplateBuilder>>,
}

impl Hyperlink {
    pub fn new(href: &str) -> Self {
        Self {
            href: href.to_string(),
            style_names: vec![],
            style_override: Default::default(),
            children: vec![],
        }
    }

    pub fn child(mut self, child: impl TemplateBuilder + 'static) -> Self {
        self.children.push(Box::new(child));
        self
    }

    pub fn style_name(mut self, name: &str) -> Self {
        self.style_names.push(name.to_string());
        self
    }

    pub fn text(self, content: &str) -> Self {
        self.child(Text::new(content))
    }
}

impl TemplateBuilder for Hyperlink {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::Hyperlink(JsonHyperlink {
            href: self.href,
            style_names: self.style_names,
            style_override: self.style_override,
            children: self.children.into_iter().map(|c| c.build()).collect(),
        }))
    }
}

/// Builder for an `<InlineImage>` node.
#[derive(Clone)]
pub struct InlineImage {
    src: String,
    style_names: Vec<String>,
    style_override: ElementStyle,
}

impl InlineImage {
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
}

impl TemplateBuilder for InlineImage {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::InlineImage(JsonImage {
            src: self.src,
            style_names: self.style_names,
            style_override: self.style_override,
        }))
    }
}

impl_styled_widget!(Span, Hyperlink, InlineImage);