use crate::builders::{InlineImage, Span, Text};
use crate::node::TemplateBuilder;
use crate::style::impl_styled_widget;
use petty_json_template::ast::{JsonNode, JsonParagraph, TemplateNode};
use petty_style::stylesheet::ElementStyle;

/// Builder for a `<Paragraph>` node.
#[derive(Default, Clone)]
pub struct Paragraph {
    id: Option<String>,
    style_names: Vec<String>,
    style_override: ElementStyle,
    children: Vec<Box<dyn TemplateBuilder>>,
}

impl Paragraph {
    /// Creates a new Paragraph containing an initial piece of content.
    /// The content can be a `&str`, `Text`, or `Span`.
    pub fn new<T: Into<Box<dyn TemplateBuilder>>>(content: T) -> Self {
        Self {
            id: None,
            children: vec![content.into()],
            ..Default::default()
        }
    }

    /// Creates a new, empty Paragraph.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Adds a child node (e.g., `Text`, `Span`, `Hyperlink`).
    pub fn child(mut self, child: impl TemplateBuilder + 'static) -> Self {
        self.children.push(Box::new(child));
        self
    }

    pub fn style_name(mut self, name: &str) -> Self {
        self.style_names.push(name.to_string());
        self
    }

    /// Convenience method to add a simple text node.
    pub fn text(self, content: &str) -> Self {
        self.child(Text::new(content))
    }

    /// Convenience method to add a styled span.
    pub fn span(self, span: Span) -> Self {
        self.child(span)
    }

    /// Convenience method to add an inline image.
    pub fn image(self, image: InlineImage) -> Self {
        self.child(image)
    }
}

impl TemplateBuilder for Paragraph {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::Paragraph(JsonParagraph {
            id: self.id,
            style_names: self.style_names,
            style_override: self.style_override,
            children: self.children.into_iter().map(|c| c.build()).collect(),
        }))
    }
}

impl_styled_widget!(Paragraph);

// Helper conversions for the ergonomic `Paragraph::new()` constructor.

impl From<&str> for Box<dyn TemplateBuilder> {
    fn from(s: &str) -> Self {
        Box::new(Text::new(s))
    }
}

impl From<String> for Box<dyn TemplateBuilder> {
    fn from(s: String) -> Self {
        Box::new(Text::new(&s))
    }
}

impl From<Text> for Box<dyn TemplateBuilder> {
    fn from(t: Text) -> Self {
        Box::new(t)
    }
}

impl From<Span> for Box<dyn TemplateBuilder> {
    fn from(s: Span) -> Self {
        Box::new(s)
    }
}
