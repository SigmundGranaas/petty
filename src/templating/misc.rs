// FILE: /home/sigmund/RustroverProjects/petty/src/core/templating/misc.rs

use crate::parser::json::ast::{JsonNode, TemplateNode};
use crate::templating::node::TemplateBuilder;

/// Builder for a `<LineBreak>` node.
#[derive(Clone, Default)]
pub struct LineBreak;

impl LineBreak {
    pub fn new() -> Self {
        Self
    }
}

impl TemplateBuilder for LineBreak {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::LineBreak)
    }
}

/// Builder for a `<PageBreak>` node.
#[derive(Clone, Default)]
pub struct PageBreak {
    master_name: Option<String>,
}

impl PageBreak {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn master_name(mut self, name: &str) -> Self {
        self.master_name = Some(name.to_string());
        self
    }
}

impl TemplateBuilder for PageBreak {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::PageBreak {
            master_name: self.master_name,
        })
    }
}

/// Builder for a `<RenderTemplate>` node.
///
/// This is used to render a named template definition that has been added to the
/// main `Template` object via the `add_definition` method.
#[derive(Clone)]
pub struct Render {
    name: String,
}

impl Render {
    /// Creates a new `Render` builder.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the template definition to render.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl TemplateBuilder for Render {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Static(JsonNode::RenderTemplate { name: self.name })
    }
}