//! A fluent, code-based API for building document templates.
//!
//! This crate provides a collection of builder structs that allow you to construct
//! a document layout programmatically, with an API designed to feel like declarative
//! UI frameworks such as Flutter.
//!
//! # Creating Reusable Components
//!
//! A powerful way to create reusable components is by using "widget functions".
//! This is a simple function that takes your component's data and returns a
//! pre-configured builder. This avoids boilerplate and is easy to compose.
//!
//! ```ignore
//! use petty_template_dsl::builders::*;
//! use petty_template_dsl::{Template, TemplateBuilder};
//!
//! // A widget function that creates a titled section.
//! fn titled_section(title: &str, child: impl TemplateBuilder + 'static) -> Block {
//!     Block::new()
//!         .style_name("section")
//!         .child(
//!             Paragraph::new(title)
//!                 .style_name("title")
//!         )
//!         .child(child)
//! }
//!
//! // Now `titled_section` can be used just like any other builder.
//! let my_template = Template::new(
//!     Block::new()
//!         .child(titled_section("My Title", Paragraph::new("Content...")))
//! );
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
mod style;
mod table;
mod template;
mod text;
mod widgets;

#[cfg(test)]
mod tests;

/// Contains all the building blocks for creating a template.
///
/// Import with `use petty_template_dsl::builders::*;` for convenience.
pub mod builders {
    pub use super::block::{Block, Flex, ListItem};
    pub use super::control::{Each, If};
    pub use super::image::Image;
    pub use super::list::List;
    pub use super::misc::{LineBreak, PageBreak, Render};
    pub use super::paragraph::Paragraph;
    pub use super::style::StyledWidget;
    pub use super::table::{Cell, Column, Row, Table};
    pub use super::text::{Hyperlink, InlineImage, Span, Text};
}

pub use self::node::TemplateBuilder;
pub use self::template::Template;
pub use self::widgets::*;
