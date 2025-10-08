// FILE: /home/sigmund/RustroverProjects/petty/src/core/templating/node.rs

use crate::parser::json::ast::TemplateNode;

/// A helper trait for cloning trait objects of `TemplateBuilder`.
pub trait CloneTemplateBuilder {
    fn clone_box(&self) -> Box<dyn TemplateBuilder>;
}

impl<T> CloneTemplateBuilder for T
where
    T: 'static + TemplateBuilder + Clone,
{
    fn clone_box(&self) -> Box<dyn TemplateBuilder> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn TemplateBuilder> {
    fn clone(&self) -> Box<dyn TemplateBuilder> {
        self.clone_box()
    }
}

/// The central trait for all template builder structs.
///
/// It allows for polymorphic composition, enabling different builders
/// (like `Paragraph` or `Block`) to be stored in a `Vec` as children.
pub trait TemplateBuilder: CloneTemplateBuilder + Send + Sync {
    /// Consumes the builder and returns a serializable `TemplateNode` from the JSON AST.
    fn build(self: Box<Self>) -> TemplateNode;
}