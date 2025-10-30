// src/core/idf/mod.rs
// FILE: src/core/idf/mod.rs

//! Defines the Intermediate Representation (IR) for the Petty PDF engine.
//!
//! The IR is a semantic layout tree (`IRNode`) that represents a single, self-contained
//! document chunk, known as a `sequence`. This tree is the canonical data structure
//! passed from the Parsing stage to the Layout stage. It contains all structural and
//! styling information but is explicitly geometry-agnostic, lacking any X/Y coordinates.
//! The layout engine's primary role is to process this tree, annotate it with
//! measurements, and then generate positioned elements for rendering.

use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::ElementStyle;
use std::fmt;
use std::sync::Arc;

/// A thread-safe, shared byte buffer, typically used for resource data like images.
pub type SharedData = Arc<Vec<u8>>;

/// Metadata common to all block-level nodes.
#[derive(Clone, Default)]
pub struct NodeMetadata {
    /// A unique identifier for this node, used as an anchor target.
    pub id: Option<String>,
    /// Pre-resolved, shared pointers to named styles.
    pub style_sets: Vec<Arc<ElementStyle>>,
    /// Parsed inline style override.
    pub style_override: Option<ElementStyle>,
}

/// Metadata common to all inline-level nodes.
#[derive(Clone, Default)]
pub struct InlineMetadata {
    /// Pre-resolved, shared pointers to named styles.
    pub style_sets: Vec<Arc<ElementStyle>>,
    /// Parsed inline style override.
    pub style_override: Option<ElementStyle>,
}

/// Helper macro to generate repetitive style accessor methods.
macro_rules! impl_style_accessors {
    (for $T:ty, $($variant:path),+) => {
        impl $T {
            /// Returns the pre-resolved, shared pointers to named styles.
            pub(crate) fn style_sets(&self) -> &[Arc<ElementStyle>] {
                match self {
                    $(
                        $variant { meta, .. } => &meta.style_sets,
                    )*
                    _ => &[],
                }
            }

            /// Returns the parsed inline style override of the node, if it has one.
            pub(crate) fn style_override(&self) -> Option<&ElementStyle> {
                match self {
                    $(
                        $variant { meta, .. } => meta.style_override.as_ref(),
                    )*
                    _ => None,
                }
            }
        }
    };
}

/// The primary enum representing all possible block-level elements in a document layout.
/// This forms the backbone of the `IRNode` tree.
#[derive(Clone)]
pub enum IRNode {
    /// The structural root for a `sequence`'s content.
    Root(Vec<IRNode>),

    /// A generic block-level container, analogous to an HTML `<div>`.
    Block { meta: NodeMetadata, children: Vec<IRNode> },

    /// A container that lays out its children horizontally.
    FlexContainer { meta: NodeMetadata, children: Vec<IRNode> },

    /// A paragraph, which is a specialized block container that can only hold
    /// inline-level content (`InlineNode`). It serves as the primary context
    /// for text wrapping and line breaking.
    Paragraph { meta: NodeMetadata, children: Vec<InlineNode> },

    /// A block-level image element.
    Image { meta: NodeMetadata, src: String },

    /// An ordered or unordered list container. Its children should exclusively be `ListItem` nodes.
    List {
        meta: NodeMetadata,
        /// The starting number for an ordered list. Defaults to 1.
        start: Option<usize>,
        /// Children are expected to be `IRNode::ListItem`.
        children: Vec<IRNode>,
    },

    /// A single item within a `List`.
    ListItem { meta: NodeMetadata, children: Vec<IRNode> },

    /// A highly structured table node that enforces a clear component hierarchy.
    Table {
        meta: NodeMetadata,
        columns: Vec<TableColumnDefinition>,
        header: Option<Box<TableHeader>>,
        body: Box<TableBody>,
    },

    /// A semantic heading element.
    Heading { meta: NodeMetadata, level: u8, children: Vec<InlineNode> },

    /// A placeholder for a generated table of contents.
    TableOfContents { meta: NodeMetadata },

    /// Inserts a hard page break, optionally switching to a new page master.
    /// If `master_name` is None, the current page master is used for the next page.
    PageBreak { master_name: Option<String> },
}

impl fmt::Debug for IRNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IRNode::Root(children) => f.debug_tuple("Root").field(children).finish(),
            IRNode::Block { meta, children } => {
                let mut dbg = f.debug_struct("Block");
                dbg.field("meta", meta);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            IRNode::FlexContainer { meta, children } => {
                let mut dbg = f.debug_struct("FlexContainer");
                dbg.field("meta", meta);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            IRNode::Paragraph { meta, children } => {
                let mut dbg = f.debug_struct("Paragraph");
                dbg.field("meta", meta);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            IRNode::Image { meta, src } => {
                let mut dbg = f.debug_struct("Image");
                dbg.field("src", src);
                dbg.field("meta", meta);
                dbg.finish()
            }
            IRNode::List { meta, start, children } => {
                let mut dbg = f.debug_struct("List");
                dbg.field("meta", meta);
                if let Some(val) = start {
                    dbg.field("start", val);
                }
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            IRNode::ListItem { meta, children } => {
                let mut dbg = f.debug_struct("ListItem");
                dbg.field("meta", meta);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            IRNode::Table { meta, columns, header, body } => {
                let mut dbg = f.debug_struct("Table");
                dbg.field("meta", meta);
                if !columns.is_empty() {
                    dbg.field("columns", columns);
                }
                if let Some(val) = header {
                    dbg.field("header", val);
                }
                dbg.field("body", body);
                dbg.finish()
            }
            IRNode::Heading { meta, level, children } => {
                let mut dbg = f.debug_struct("Heading");
                dbg.field("meta", meta);
                dbg.field("level", level);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            IRNode::TableOfContents { meta } => {
                f.debug_struct("TableOfContents").field("meta", meta).finish()
            }
            IRNode::PageBreak { master_name } => {
                let mut dbg = f.debug_struct("PageBreak");
                if let Some(val) = master_name {
                    dbg.field("master_name", val);
                }
                dbg.finish()
            }
        }
    }
}

impl fmt::Debug for NodeMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("NodeMetadata");
        if let Some(id) = &self.id {
            dbg.field("id", id);
        }
        if !self.style_sets.is_empty() {
            dbg.field("style_sets", &self.style_sets);
        }
        if let Some(val) = &self.style_override {
            dbg.field("style_override", val);
        }
        dbg.finish()
    }
}

impl_style_accessors!(
    for IRNode,
    IRNode::Block,
    IRNode::FlexContainer,
    IRNode::Paragraph,
    IRNode::Image,
    IRNode::List,
    IRNode::ListItem,
    IRNode::Table,
    IRNode::Heading,
    IRNode::TableOfContents
);

// --- Table Component Structs ---

/// Represents the definition of a single column in a table, containing
/// information needed for layout calculation.
#[derive(Clone, PartialEq, Default)]
pub struct TableColumnDefinition {
    pub width: Option<Dimension>,
    // These string styles will be resolved by the TreeBuilder, not the layout engine.
    pub style: Option<String>,
    pub header_style: Option<String>,
}

impl fmt::Debug for TableColumnDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("TableColumnDefinition");
        if let Some(val) = &self.width {
            dbg.field("width", val);
        }
        if let Some(val) = &self.style {
            dbg.field("style", val);
        }
        if let Some(val) = &self.header_style {
            dbg.field("header_style", val);
        }
        dbg.finish()
    }
}

/// A container for the header rows of a `Table`.
#[derive(Debug, Clone)]
pub struct TableHeader {
    pub rows: Vec<TableRow>,
}

/// A container for the body rows of a `Table`.
#[derive(Debug, Clone, Default)]
pub struct TableBody {
    pub rows: Vec<TableRow>,
}

/// Represents a single row within a `Table`, containing a vector of `TableCell`s.
#[derive(Debug, Clone)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

/// Represents a single cell within a `TableRow`. A cell can contain any
/// block-level `IRNode` elements, allowing for complex nested layouts.
#[derive(Clone)]
pub struct TableCell {
    pub style_sets: Vec<Arc<ElementStyle>>,
    pub style_override: Option<ElementStyle>,
    /// Cell content is block-level, allowing for complex nested layouts.
    pub children: Vec<IRNode>,
    pub colspan: usize,
    pub rowspan: usize,
}

impl fmt::Debug for TableCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("TableCell");
        if !self.style_sets.is_empty() {
            dbg.field("style_sets", &self.style_sets);
        }
        if let Some(val) = &self.style_override {
            dbg.field("style_override", val);
        }
        if !self.children.is_empty() {
            dbg.field("children", &self.children);
        }
        if self.colspan != 1 {
            dbg.field("colspan", &self.colspan);
        }
        if self.rowspan != 1 {
            dbg.field("rowspan", &self.rowspan);
        }
        dbg.finish()
    }
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            style_sets: Default::default(),
            style_override: Default::default(),
            children: Default::default(),
            colspan: 1,
            rowspan: 1,
        }
    }
}

// --- Inline Content Enum ---

/// Represents content that flows within a line-breaking context, such as a `Paragraph`.
#[derive(Clone)]
pub enum InlineNode {
    /// A raw string of text.
    Text(String),

    /// A styled text container, analogous to an HTML `<span>`.
    StyledSpan { meta: InlineMetadata, children: Vec<InlineNode> },

    /// An interactive hyperlink.
    Hyperlink {
        href: String,
        meta: InlineMetadata,
        children: Vec<InlineNode>,
    },

    /// An image placed inline with text.
    Image { meta: InlineMetadata, src: String },

    /// An inline placeholder for a page number that will be resolved later.
    PageReference {
        target_id: String,
        meta: InlineMetadata,
        children: Vec<InlineNode>,
    },

    /// A hard line break, analogous to an HTML `<br>`.
    LineBreak,
}

impl fmt::Debug for InlineNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InlineNode::Text(text) => f.debug_tuple("Text").field(text).finish(),
            InlineNode::StyledSpan { meta, children } => {
                let mut dbg = f.debug_struct("StyledSpan");
                dbg.field("meta", meta);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            InlineNode::Hyperlink { href, meta, children } => {
                let mut dbg = f.debug_struct("Hyperlink");
                dbg.field("href", href);
                dbg.field("meta", meta);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            InlineNode::Image { meta, src } => {
                let mut dbg = f.debug_struct("Image");
                dbg.field("src", src);
                dbg.field("meta", meta);
                dbg.finish()
            }
            InlineNode::PageReference { target_id, meta, children } => {
                let mut dbg = f.debug_struct("PageReference");
                dbg.field("target_id", target_id);
                dbg.field("meta", meta);
                if !children.is_empty() {
                    dbg.field("children", children);
                }
                dbg.finish()
            }
            InlineNode::LineBreak => write!(f, "LineBreak"),
        }
    }
}

impl fmt::Debug for InlineMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("InlineMetadata");
        if !self.style_sets.is_empty() {
            dbg.field("style_sets", &self.style_sets);
        }
        if let Some(val) = &self.style_override {
            dbg.field("style_override", val);
        }
        dbg.finish()
    }
}

impl IRNode {
    /// Returns a mutable reference to the node's metadata, if it has any.
    pub fn meta_mut(&mut self) -> Option<&mut NodeMetadata> {
        match self {
            IRNode::Block { meta, .. }
            | IRNode::FlexContainer { meta, .. }
            | IRNode::Paragraph { meta, .. }
            | IRNode::Image { meta, .. }
            | IRNode::List { meta, .. }
            | IRNode::ListItem { meta, .. }
            | IRNode::Table { meta, .. }
            | IRNode::Heading { meta, .. }
            | IRNode::TableOfContents { meta, .. } => Some(meta),
            _ => None,
        }
    }
}