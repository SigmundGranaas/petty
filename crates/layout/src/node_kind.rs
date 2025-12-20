use petty_idf::IRNode;

/// Represents the specific type of a layout node.
///
/// This enum replaces stringly-typed identifiers for node lookups and registration,
/// providing compile-time safety and faster comparisons/hashing during layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Root,
    Block,
    Paragraph,
    Heading,
    Image,
    FlexContainer,
    List,
    ListItem,
    Table,
    PageBreak,
    IndexMarker,
}

impl NodeKind {
    /// Determines the `NodeKind` from a given `IRNode`.
    pub fn from_ir(node: &IRNode) -> Self {
        match node {
            IRNode::Root(_) => NodeKind::Root,
            IRNode::Block { .. } => NodeKind::Block,
            IRNode::Paragraph { .. } => NodeKind::Paragraph,
            IRNode::Heading { .. } => NodeKind::Heading,
            IRNode::Image { .. } => NodeKind::Image,
            IRNode::FlexContainer { .. } => NodeKind::FlexContainer,
            IRNode::List { .. } => NodeKind::List,
            IRNode::ListItem { .. } => NodeKind::ListItem,
            IRNode::Table { .. } => NodeKind::Table,
            IRNode::PageBreak { .. } => NodeKind::PageBreak,
            IRNode::IndexMarker { .. } => NodeKind::IndexMarker,
        }
    }

    /// Returns a string representation, primarily for debugging or error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Root => "Root",
            NodeKind::Block => "Block",
            NodeKind::Paragraph => "Paragraph",
            NodeKind::Heading => "Heading",
            NodeKind::Image => "Image",
            NodeKind::FlexContainer => "FlexContainer",
            NodeKind::List => "List",
            NodeKind::ListItem => "ListItem",
            NodeKind::Table => "Table",
            NodeKind::PageBreak => "PageBreak",
            NodeKind::IndexMarker => "IndexMarker",
        }
    }
}
