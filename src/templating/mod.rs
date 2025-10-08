//! A fluent, code-based API for building document templates.
//!
//! This module provides a collection of builder structs that allow you to construct
//! a document layout programmatically, with an API designed to feel like declarative
//! UI frameworks such as Flutter.
//!
//! # Creating Reusable Components
//!
//! The most powerful feature of this API is the ability to create your own reusable
//! components, analogous to "Widgets" in Flutter. This is achieved by creating a
//! struct and implementing the [`TemplateBuilder`] trait for it.
//!
//! ```
//! # use petty::templating::builders::*;
//! # use petty::templating::TemplateBuilder;
//! # use petty::parser::json::ast::TemplateNode;
//! #[derive(Clone)]
//! struct TitledSection {
//!     title: String,
//!     child: Box<dyn TemplateBuilder>,
//! }
//!
//! impl TitledSection {
//!     pub fn new(title: &str, child: impl TemplateBuilder + 'static) -> Self {
//!         Self { title: title.to_string(), child: Box::new(child) }
//!     }
//! }
//!
//! impl TemplateBuilder for TitledSection {
//!     fn build(self: Box<Self>) -> TemplateNode {
//!         let component = Block::new()
//!             .style_name("section")
//!             .child(Paragraph::new().style_name("title").text(&self.title))
//!             .child(self.child);
//!
//!         // A component's build method returns another builder, which we then build.
//!         Box::new(component).build()
//!     }
//! }
//!
//! // Now `TitledSection` can be used just like any other builder.
//! let my_component = TitledSection::new("My Title", Paragraph::new().text("Content..."));
//! ```
//!
//! The end goal of using these builders is to produce a [`Template`] object, which can
//! then be serialized into the standard JSON template format for processing by the
//! engine.

mod block;
mod control;
mod image;
mod list;
mod misc;
mod node;
mod paragraph;
mod table;
mod template;
mod text;

#[cfg(test)]
mod tests;

/// Contains all the building blocks for creating a template.
///
/// Import with `use petty::templating::builders::*;` for convenience.
pub mod builders {
    pub use super::block::{Block, Flex, ListItem};
    pub use super::control::{Each, If};
    pub use super::image::Image;
    pub use super::list::List;
    pub use super::misc::{LineBreak, PageBreak, Render};
    pub use super::paragraph::Paragraph;
    pub use super::table::{Cell, Column, Row, Table};
    pub use super::text::{Hyperlink, InlineImage, Span, Text};
}

pub use self::node::TemplateBuilder;
pub use self::template::Template;