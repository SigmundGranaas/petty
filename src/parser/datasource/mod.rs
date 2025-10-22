// FILE: src/parser/datasource/mod.rs
//! Defines the core abstraction for a navigable, read-only data source tree.
use std::hash::{Hash};

/// A qualified name, consisting of an optional prefix and a local part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QName<'a> {
    pub prefix: Option<&'a str>,
    pub local_part: &'a str,
}

/// The type of a node in the data source tree, aligned with the XPath 1.0 data model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    Root,
    Element,
    Attribute,
    Text,
}

/// The universal contract for a node in a read-only, hierarchical data source.
///
/// This trait is the heart of the decoupled architecture. The XPath and XSLT engines
/// are written exclusively against this trait, allowing them to operate on any data
/// source (XML, JSON VDOM, etc.) that implements it.
///
/// `'a` is the lifetime of the underlying data source (e.g., the XML string).
pub trait DataSourceNode<'a>: std::fmt::Debug + Clone + Copy + PartialEq + Eq + Hash {
    /// The type of the node (Element, Text, Attribute, etc.).
    fn node_type(&self) -> NodeType;

    /// The qualified name of the node (e.g., `fo:block`). Returns `None` for node
    /// types that do not have names, such as text or root nodes.
    fn name(&self) -> Option<QName<'a>>;

    /// The string value of the node, as defined by the XPath 1.0 `string()` function.
    /// - For a text node, this is its content.
    /// - For an element, this is the concatenation of the string values of all
    ///   its descendant text nodes.
    /// - For an attribute, this is its value.
    fn string_value(&self) -> String;

    /// An iterator over the attribute nodes of this node.
    /// The iterator will be empty for non-element nodes.
    fn attributes(&self) -> Box<dyn Iterator<Item = Self> + 'a>;

    /// An iterator over the child nodes of this node.
    /// The iterator will be empty for leaf nodes (like text or attributes).
    fn children(&self) -> Box<dyn Iterator<Item = Self> + 'a>;

    /// A reference to the parent node. Returns `None` for the root node or attributes detached
    /// from an element. This is essential for axes like `parent::` and `ancestor::`.
    fn parent(&self) -> Option<Self>;
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::hash::Hasher;

    // --- Mock Implementation for TDD ---

    #[derive(Debug, Clone)]
    struct MockNodeData<'a> {
        node_type: NodeType,
        name: Option<QName<'a>>,
        value: String,
        children: Vec<usize>,
        attributes: Vec<usize>,
    }

    #[derive(Debug)]
    pub struct MockTree<'a> {
        nodes: HashMap<usize, MockNodeData<'a>>,
        // We need a way to map a child ID back to its parent ID for the parent() method.
        parent_map: HashMap<usize, usize>,
    }

    /// A simple, in-memory node representation that holds a reference to its tree.
    /// This is necessary so that the node can navigate itself (e.g., find its parent or children).
    #[derive(Debug, Clone, Copy)]
    pub struct MockNode<'a> {
        pub id: usize,
        pub tree: &'a MockTree<'a>,
    }

    impl<'a> PartialEq for MockNode<'a> {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl<'a> Eq for MockNode<'a> {}

    impl<'a> Hash for MockNode<'a> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }

    impl<'a> DataSourceNode<'a> for MockNode<'a> {
        fn node_type(&self) -> NodeType {
            self.tree.nodes[&self.id].node_type
        }

        fn name(&self) -> Option<QName<'a>> {
            self.tree.nodes[&self.id].name
        }

        fn string_value(&self) -> String {
            self.tree.nodes[&self.id].value.clone()
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
            let tree = self.tree; // Re-borrow to help the lifetime checker
            let attribute_ids = tree.nodes[&self.id].attributes.clone();
            Box::new(attribute_ids.into_iter().map(move |id| MockNode { id, tree }))
        }

        fn children(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
            let tree = self.tree; // Re-borrow to help the lifetime checker
            let children_ids = tree.nodes[&self.id].children.clone();
            Box::new(children_ids.into_iter().map(move |id| MockNode { id, tree }))
        }

        fn parent(&self) -> Option<Self> {
            self.tree.parent_map.get(&self.id).map(|&pid| MockNode { id: pid, tree: self.tree })
        }
    }

    /// Creates a simple mock tree for testing:
    /// <root> <!-- id 0 -->
    ///   <para id="p1">Hello</para> <!-- id 1, attr 2, text 3 -->
    /// </root>
    pub fn create_test_tree<'a>() -> MockTree<'a> {
        let mut nodes = HashMap::new();
        let mut parent_map = HashMap::new();

        nodes.insert(0, MockNodeData {
            node_type: NodeType::Root,
            name: None,
            value: "Hello".to_string(),
            children: vec![1],
            attributes: vec![],
        });
        nodes.insert(1, MockNodeData {
            node_type: NodeType::Element,
            name: Some(QName { prefix: None, local_part: "para" }),
            value: "Hello".to_string(),
            children: vec![3],
            attributes: vec![2],
        });
        parent_map.insert(1, 0);

        nodes.insert(2, MockNodeData {
            node_type: NodeType::Attribute,
            name: Some(QName { prefix: None, local_part: "id" }),
            value: "p1".to_string(),
            children: vec![],
            attributes: vec![],
        });
        parent_map.insert(2, 1);

        nodes.insert(3, MockNodeData {
            node_type: NodeType::Text,
            name: None,
            value: "Hello".to_string(),
            children: vec![],
            attributes: vec![],
        });
        parent_map.insert(3, 1);

        MockTree { nodes, parent_map }
    }
}