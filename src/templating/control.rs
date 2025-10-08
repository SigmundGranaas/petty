// FILE: /home/sigmund/RustroverProjects/petty/src/core/templating/control.rs

use crate::parser::json::ast::{ControlNode, TemplateNode};
use crate::templating::node::TemplateBuilder;

/// Builder for an `{{#if}}` control flow block.
#[derive(Clone)]
pub struct If {
    test: String,
    then_branch: Box<dyn TemplateBuilder>,
    else_branch: Option<Box<dyn TemplateBuilder>>,
}

impl If {
    /// Creates a new `If` builder.
    ///
    /// # Arguments
    ///
    /// * `test` - A Handlebars expression (without `{{` or `}}`) that evaluates to a boolean.
    /// * `then_branch` - The template builder to render if the condition is true.
    pub fn new(test: &str, then_branch: impl TemplateBuilder + 'static) -> Self {
        Self {
            test: test.to_string(),
            then_branch: Box::new(then_branch),
            else_branch: None,
        }
    }

    /// Sets the template to render if the condition is false.
    pub fn with_else(mut self, else_branch: impl TemplateBuilder + 'static) -> Self {
        self.else_branch = Some(Box::new(else_branch));
        self
    }
}

impl TemplateBuilder for If {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Control(ControlNode::If {
            test: self.test,
            then: Box::new(self.then_branch.build()),
            else_branch: self.else_branch.map(|b| Box::new(b.build())),
        })
    }
}

/// Builder for an `{{#each}}` control flow block.
#[derive(Clone)]
pub struct Each {
    each_path: String,
    template: Box<dyn TemplateBuilder>,
}

impl Each {
    /// Creates a new `Each` builder.
    ///
    /// # Arguments
    ///
    /// * `each_path` - A JSON pointer or Handlebars path resolving to an array in the data context.
    /// * `template` - The template builder to instantiate for each item in the array.
    pub fn new(each_path: &str, template: impl TemplateBuilder + 'static) -> Self {
        Self {
            each_path: each_path.to_string(),
            template: Box::new(template),
        }
    }
}

impl TemplateBuilder for Each {
    fn build(self: Box<Self>) -> TemplateNode {
        TemplateNode::Control(ControlNode::Each {
            each: self.each_path,
            template: Box::new(self.template.build()),
        })
    }
}