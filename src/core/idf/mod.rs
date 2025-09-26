//! Defines the Intermediate Representation (IR) for the Petty PDF engine.
//!
//! The IR is a semantic layout tree (`IRNode`) that represents a single, self-contained
//! document chunk, known as a `sequence`. This tree is the canonical data structure
//! passed from the Parsing stage to the Layout stage. It contains all structural and
//! styling information but is explicitly geometry-agnostic, lacking any X/Y coordinates.
//! The layout engine's primary role is to process this tree, annotate it with
//! measurements, and then generate positioned elements for rendering.

use serde_json::Value;
use std::sync::Arc;
use crate::core::style::dimension::Dimension;
use crate::core::style::stylesheet::ElementStyle;

/// A thread-safe, shared byte buffer, typically used for resource data like images.
pub type SharedData = Arc<Vec<u8>>;

/// Represents the top-level unit of work for the layout engine. It pairs a complete
/// `IRNode` tree with the specific data context that was used to generate it.
pub struct LayoutUnit {
    /// The root of the layout tree for a single `sequence`.
    pub tree: IRNode,
    /// A thread-safe reference to the JSON data context for this specific `sequence`.
    pub context: Arc<Value>,
}

/// The primary enum representing all possible block-level elements in a document layout.
/// This forms the backbone of the `IRNode` tree.
#[derive(Debug, Clone)]
pub enum IRNode {
    /// The structural root for a `sequence`'s content.
    Root(Vec<IRNode>),

    /// A generic block-level container, analogous to an HTML `<div>`.
    Block {
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        children: Vec<IRNode>,
    },

    /// A container that lays out its children horizontally.
    FlexContainer {
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        children: Vec<IRNode>,
    },

    /// A paragraph, which is a specialized block container that can only hold
    /// inline-level content (`InlineNode`). It serves as the primary context
    /// for text wrapping and line breaking.
    Paragraph {
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        children: Vec<InlineNode>,
    },

    /// A block-level image element.
    Image {
        src: String,
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
    },

    /// An ordered or unordered list container. Its children should exclusively be `ListItem` nodes.
    List {
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        /// Children are expected to be `IRNode::ListItem`.
        children: Vec<IRNode>,
    },

    /// A single item within a `List`.
    ListItem {
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        children: Vec<IRNode>,
    },

    /// A highly structured table node that enforces a clear component hierarchy.
    Table {
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        columns: Vec<TableColumnDefinition>,
        /// This field is populated by the Layout Engine's measurement pass.
        calculated_widths: Vec<f32>,
        header: Option<Box<TableHeader>>,
        body: Box<TableBody>,
    },
}

impl IRNode {
    /// Returns the pre-resolved, shared pointers to named styles.
    pub(crate) fn style_sets(&self) -> &[Arc<ElementStyle>] {
        match self {
            IRNode::Block { style_sets, .. }
            | IRNode::FlexContainer { style_sets, .. }
            | IRNode::Paragraph { style_sets, .. }
            | IRNode::Image { style_sets, .. }
            | IRNode::List { style_sets, .. }
            | IRNode::ListItem { style_sets, .. }
            | IRNode::Table { style_sets, .. } => style_sets,
            IRNode::Root(_) => &[],
        }
    }

    /// Returns the parsed inline style override of the node, if it has one.
    pub(crate) fn style_override(&self) -> Option<&ElementStyle> {
        match self {
            IRNode::Block { style_override, .. }
            | IRNode::FlexContainer { style_override, .. }
            | IRNode::Paragraph { style_override, .. }
            | IRNode::Image { style_override, .. }
            | IRNode::List { style_override, .. }
            | IRNode::ListItem { style_override, .. }
            | IRNode::Table { style_override, .. } => style_override.as_ref(),
            IRNode::Root(_) => None,
        }
    }
}

// --- Table Component Structs ---

/// Represents the definition of a single column in a table, containing
/// information needed for layout calculation.
#[derive(Debug, Clone, PartialEq, Default)] // Added Default derive
pub struct TableColumnDefinition {
    pub width: Option<Dimension>,
    // These string styles will be resolved by the TreeBuilder, not the layout engine.
    pub style: Option<String>,
    pub header_style: Option<String>,
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
/// block-level `IRNode` elements, allowing for nested structures.
#[derive(Debug, Clone)]
pub struct TableCell {
    pub style_sets: Vec<Arc<ElementStyle>>,
    pub style_override: Option<ElementStyle>,
    /// Cell content is block-level, allowing for complex nested layouts.
    pub children: Vec<IRNode>,
}

// --- Inline Content Enum ---

/// Represents content that flows within a line-breaking context, such as a `Paragraph`.
#[derive(Debug, Clone)]
pub enum InlineNode {
    /// A raw string of text.
    Text(String),

    /// A styled text container, analogous to an HTML `<span>`.
    StyledSpan {
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        children: Vec<InlineNode>,
    },

    /// An interactive hyperlink.
    Hyperlink {
        href: String,
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
        children: Vec<InlineNode>,
    },

    /// An image placed inline with text.
    Image {
        src: String,
        style_sets: Vec<Arc<ElementStyle>>,
        style_override: Option<ElementStyle>,
    },

    /// A hard line break, analogous to an HTML `<br>`.
    LineBreak,
}

impl InlineNode {
    /// Returns the pre-resolved, shared pointers to named styles.
    pub(crate) fn style_sets(&self) -> &[Arc<ElementStyle>] {
        match self {
            InlineNode::StyledSpan { style_sets, .. }
            | InlineNode::Hyperlink { style_sets, .. }
            | InlineNode::Image { style_sets, .. } => style_sets,
            _ => &[],
        }
    }

    /// Returns the parsed inline style override of the inline node, if it has one.
    pub(crate) fn style_override(&self) -> Option<&ElementStyle> {
        match self {
            InlineNode::StyledSpan { style_override, .. }
            | InlineNode::Hyperlink { style_override, .. }
            | InlineNode::Image { style_override, .. } => style_override.as_ref(),
            _ => None,
        }
    }
}