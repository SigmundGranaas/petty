// src/core/idf/mod.rs
//! Intermediate Document Format (IDF)
//! This module defines the core, in-memory representation of a document's
//! structure and content after parsing but before layout.

use petty_style::dimension::Dimension;
use petty_style::stylesheet::ElementStyle;
use std::sync::Arc;

// --- Shared Types ---

/// A string type for the document.
/// Note: Switched to String to match parser output.
pub type TextStr = String;

/// A reference-counted container for shared, immutable data like images.
pub type SharedData = Arc<Vec<u8>>;

/// A common metadata structure for all block-level `IRNode`s.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeMetadata {
    pub id: Option<TextStr>,
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
    Block {
        meta: NodeMetadata,
        children: Vec<IRNode>,
    },
    /// A paragraph, containing only inline content.
    Paragraph {
        meta: NodeMetadata,
        children: Vec<InlineNode>,
    },
    /// A heading, with a level and inline content.
    Heading {
        meta: NodeMetadata,
        level: u8,
        children: Vec<InlineNode>,
    },
    /// An image.
    Image { meta: NodeMetadata, src: TextStr },
    /// A container for flexible box layout.
    FlexContainer {
        meta: NodeMetadata,
        children: Vec<IRNode>,
    },
    /// An ordered or unordered list.
    List {
        meta: NodeMetadata,
        start: Option<usize>,
        children: Vec<IRNode>,
    },
    /// An item within a list.
    ListItem {
        meta: NodeMetadata,
        children: Vec<IRNode>,
    },
    /// A table.
    Table {
        meta: NodeMetadata,
        columns: Vec<TableColumnDefinition>,
        header: Option<Box<TableHeader>>,
        body: Box<TableBody>,
    },
    /// A hard page break.
    PageBreak { master_name: Option<TextStr> },
    /// A marker for generating an index entry, with no visual output.
    IndexMarker { meta: NodeMetadata, term: TextStr },
}

impl IRNode {
    /// Returns a reference to the metadata if the node type supports it.
    pub fn meta(&self) -> Option<&NodeMetadata> {
        match self {
            IRNode::Block { meta, .. } => Some(meta),
            IRNode::Paragraph { meta, .. } => Some(meta),
            IRNode::Heading { meta, .. } => Some(meta),
            IRNode::Image { meta, .. } => Some(meta),
            IRNode::FlexContainer { meta, .. } => Some(meta),
            IRNode::List { meta, .. } => Some(meta),
            IRNode::ListItem { meta, .. } => Some(meta),
            IRNode::Table { meta, .. } => Some(meta),
            IRNode::IndexMarker { meta, .. } => Some(meta),
            IRNode::Root(_) | IRNode::PageBreak { .. } => None,
        }
    }

    /// Returns a mutable reference to the metadata if the node type supports it.
    pub fn meta_mut(&mut self) -> Option<&mut NodeMetadata> {
        match self {
            IRNode::Block { meta, .. } => Some(meta),
            IRNode::Paragraph { meta, .. } => Some(meta),
            IRNode::Heading { meta, .. } => Some(meta),
            IRNode::Image { meta, .. } => Some(meta),
            IRNode::FlexContainer { meta, .. } => Some(meta),
            IRNode::List { meta, .. } => Some(meta),
            IRNode::ListItem { meta, .. } => Some(meta),
            IRNode::Table { meta, .. } => Some(meta),
            IRNode::IndexMarker { meta, .. } => Some(meta),
            IRNode::Root(_) | IRNode::PageBreak { .. } => None,
        }
    }

    pub fn style_sets(&self) -> &[Arc<ElementStyle>] {
        self.meta().map(|m| m.style_sets.as_slice()).unwrap_or(&[])
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
    Text(TextStr),
    /// A styled `<span>`.
    StyledSpan {
        meta: InlineMetadata,
        children: Vec<InlineNode>,
    },
    /// A hyperlink `<a>`.
    Hyperlink {
        meta: InlineMetadata,
        href: TextStr,
        children: Vec<InlineNode>,
    },
    /// A cross-reference to another page.
    PageReference {
        meta: InlineMetadata,
        target_id: TextStr,
        children: Vec<InlineNode>,
    },
    /// An inline image.
    Image { meta: InlineMetadata, src: TextStr },
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
    pub width: Option<Dimension>,
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
