// src/core/idf.rs
//! Intermediate Document Format (IDF)
//! This module defines the core, in-memory representation of a document's
//! structure and content after parsing but before layout. It is a tree-like
//! structure composed of `IRNode` and `InlineNode` enums.

use crate::core::style::stylesheet::ElementStyle;
use std::sync::Arc;

// --- Shared Types ---

/// A reference-counted container for shared, immutable data like images.
pub type SharedData = Arc<Vec<u8>>;

/// A common metadata structure for all block-level `IRNode`s.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeMetadata {
    pub id: Option<String>,
    pub style_sets: Vec<Arc<ElementStyle>>,
    pub style_override: Option<ElementStyle>,
}

/// A common metadata structure for all `InlineNode`s.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InlineMetadata {
    pub style_sets: Vec<Arc<ElementStyle>>,
    pub style_override: Option<ElementStyle>,
}

// --- Main Node Enums ---

/// Represents a block-level element in the document tree.
#[derive(Debug, Clone, PartialEq)]
pub enum IRNode {
    /// The root of a document fragment, containing other block nodes.
    Root(Vec<IRNode>),
    /// A generic block container.
    Block { meta: NodeMetadata, children: Vec<IRNode> },
    /// A paragraph, containing only inline content.
    Paragraph { meta: NodeMetadata, children: Vec<InlineNode> },
    /// A heading, with a level and inline content.
    Heading { meta: NodeMetadata, level: u8, children: Vec<InlineNode> },
    /// An image.
    Image { meta: NodeMetadata, src: String },
    /// A container for flexible box layout.
    FlexContainer { meta: NodeMetadata, children: Vec<IRNode> },
    /// An ordered or unordered list.
    List { meta: NodeMetadata, start: Option<usize>, children: Vec<IRNode> },
    /// An item within a list.
    ListItem { meta: NodeMetadata, children: Vec<IRNode> },
    /// A table.
    Table {
        meta: NodeMetadata,
        columns: Vec<TableColumnDefinition>,
        header: Option<Box<TableHeader>>,
        body: Box<TableBody>,
    },
    /// A hard page break.
    PageBreak { master_name: Option<String> },
    /// A marker for generating an index entry, with no visual output.
    IndexMarker { meta: NodeMetadata, term: String },
}

impl IRNode {
    pub fn meta(&self) -> Option<&NodeMetadata> {
        match self {
            IRNode::Block { meta, .. }
            | IRNode::Paragraph { meta, .. }
            | IRNode::Heading { meta, .. }
            | IRNode::Image { meta, .. }
            | IRNode::FlexContainer { meta, .. }
            | IRNode::List { meta, .. }
            | IRNode::ListItem { meta, .. }
            | IRNode::Table { meta, .. }
            | IRNode::IndexMarker { meta, .. } => Some(meta),
            _ => None,
        }
    }

    pub fn meta_mut(&mut self) -> Option<&mut NodeMetadata> {
        match self {
            IRNode::Block { meta, .. }
            | IRNode::Paragraph { meta, .. }
            | IRNode::Heading { meta, .. }
            | IRNode::Image { meta, .. }
            | IRNode::FlexContainer { meta, .. }
            | IRNode::List { meta, .. }
            | IRNode::ListItem { meta, .. }
            | IRNode::Table { meta, .. }
            | IRNode::IndexMarker { meta, .. } => Some(meta),
            _ => None,
        }
    }

    pub fn style_sets(&self) -> &[Arc<ElementStyle>] {
        self.meta()
            .map(|m| m.style_sets.as_slice())
            .unwrap_or(&[])
    }

    pub fn style_override(&self) -> Option<&ElementStyle> {
        self.meta().and_then(|m| m.style_override.as_ref())
    }

    /// Returns a string identifier for the node type, used for dynamic dispatch in the layout engine.
    pub fn kind(&self) -> &'static str {
        match self {
            IRNode::Root(_) => "root",
            IRNode::Block { .. } => "block",
            IRNode::Paragraph { .. } => "paragraph",
            IRNode::Heading { .. } => "heading",
            IRNode::Image { .. } => "image",
            IRNode::FlexContainer { .. } => "flex-container",
            IRNode::List { .. } => "list",
            IRNode::ListItem { .. } => "list-item",
            IRNode::Table { .. } => "table",
            IRNode::PageBreak { .. } => "page-break",
            IRNode::IndexMarker { .. } => "index-marker",
        }
    }
}


/// Represents an inline-level element within a block like a `Paragraph`.
#[derive(Debug, Clone, PartialEq)]
pub enum InlineNode {
    /// A run of plain text.
    Text(String),
    /// A styled `<span>`.
    StyledSpan { meta: InlineMetadata, children: Vec<InlineNode> },
    /// A hyperlink `<a>`.
    Hyperlink { meta: InlineMetadata, href: String, children: Vec<InlineNode> },
    /// A cross-reference to another page.
    PageReference { meta: InlineMetadata, target_id: String, children: Vec<InlineNode> },
    /// An inline image.
    Image { meta: InlineMetadata, src: String },
    /// A soft line break.
    LineBreak,
}

// --- Table-specific Structures ---

#[derive(Debug, Clone, PartialEq)]
pub struct TableNode {
    pub meta: NodeMetadata,
    pub columns: Vec<TableColumnDefinition>,
    pub header: Option<Box<TableHeader>>,
    pub body: Box<TableBody>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableColumnDefinition {
    pub width: Option<crate::core::style::dimension::Dimension>,
    pub style: Option<ElementStyle>,
    pub header_style: Option<ElementStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableHeader {
    pub rows: Vec<TableRow>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableBody {
    pub rows: Vec<TableRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableCell {
    pub style_sets: Vec<Arc<ElementStyle>>,
    pub style_override: Option<ElementStyle>,
    pub children: Vec<IRNode>,
    pub col_span: usize,
    pub row_span: usize,
}