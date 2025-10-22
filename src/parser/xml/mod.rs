// FILE: src/parser/xml/mod.rs
//! An implementation of the `DataSourceNode` trait for the `roxmltree` crate.
//! This provides a high-performance, compliant XML data source.

use crate::parser::datasource::{DataSourceNode, NodeType, QName};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};

/// A wrapper around a `roxmltree::Document` that acts as the entry point
/// for processing an XML data source.
///
/// It holds the parsed document in memory and provides access to the root node.
pub struct XmlDocument<'a> {
    doc: roxmltree::Document<'a>,
}

impl<'a> XmlDocument<'a> {
    /// Parses an XML string into a navigable document.
    pub fn parse(text: &'a str) -> Result<Self, roxmltree::Error> {
        let doc = roxmltree::Document::parse(text)?;
        Ok(XmlDocument { doc })
    }

    /// Returns the root node of the document, which corresponds to the document element.
    pub fn root_node(&'a self) -> XmlNode<'a> {
        // According to the XPath data model, the root node is the parent of the document element.
        XmlNode::Node(self.doc.root())
    }
}

/// An enum that represents any kind of node in the XML tree (element, attribute, etc.).
/// This is necessary because `roxmltree` treats `Node` and `Attribute` as distinct types,
/// but the XPath data model treats them both as nodes. This enum unifies them.
#[derive(Clone, Copy)]
pub enum XmlNode<'a> {
    Node(roxmltree::Node<'a, 'a>),
    Attribute {
        attr: roxmltree::Attribute<'a, 'a>,
        parent: roxmltree::Node<'a, 'a>,
    },
}

// Manual trait implementations are required because roxmltree's types don't derive them.
impl<'a> Debug for XmlNode<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            XmlNode::Node(e) => e.fmt(f),
            XmlNode::Attribute { attr, .. } => attr.fmt(f),
        }
    }
}

impl<'a> PartialEq for XmlNode<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (XmlNode::Node(a), XmlNode::Node(b)) => a == b,
            (XmlNode::Attribute { attr: a, .. }, XmlNode::Attribute { attr: b, .. }) => a == b,
            _ => false,
        }
    }
}
impl<'a> Eq for XmlNode<'a> {}

impl<'a> Hash for XmlNode<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            XmlNode::Node(n) => n.hash(state),
            // roxmltree::Attribute doesn't impl Hash, so we build one from its unique components.
            XmlNode::Attribute { attr, parent } => {
                parent.hash(state);
                attr.name().hash(state);
            }
        }
    }
}

impl<'a> DataSourceNode<'a> for XmlNode<'a> {
    fn node_type(&self) -> NodeType {
        match self {
            XmlNode::Node(n) if n.is_root() => NodeType::Root,
            XmlNode::Node(n) if n.is_element() => NodeType::Element,
            XmlNode::Node(n) if n.is_text() => NodeType::Text,
            XmlNode::Attribute { .. } => NodeType::Attribute,
            _ => unreachable!(), // Comments, PIs are filtered out by iterators
        }
    }

    fn name(&self) -> Option<QName<'a>> {
        match self {
            XmlNode::Node(n) if n.is_element() => {
                let tag_name = n.tag_name();
                Some(QName { prefix: tag_name.namespace(), local_part: tag_name.name() })
            }
            XmlNode::Attribute { attr, .. } => {
                Some(QName { prefix: attr.namespace(), local_part: attr.name() })
            }
            _ => None,
        }
    }

    fn string_value(&self) -> String {
        match self {
            XmlNode::Attribute { attr, .. } => attr.value().to_string(),
            XmlNode::Node(n) if n.is_text() => n.text().unwrap_or("").to_string(),
            XmlNode::Node(n) => {
                let mut s = String::new();
                for child in n.descendants() {
                    if child.is_text() {
                        if let Some(text) = child.text() {
                            s.push_str(text);
                        }
                    }
                }
                s
            }
        }
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        match self {
            XmlNode::Node(n) if n.is_element() => {
                let parent_node = *n;
                Box::new(
                    n.attributes()
                        .map(move |attr| XmlNode::Attribute { attr, parent: parent_node }),
                )
            }
            _ => Box::new(std::iter::empty()),
        }
    }

    fn children(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        match self {
            XmlNode::Node(n) => Box::new(
                n.children()
                    .filter(|c| {
                        // This filter implements an implicit `xsl:strip-space elements="*"`.
                        // It removes whitespace-only text nodes from the data source tree.
                        if c.is_element() {
                            true
                        } else if c.is_text() {
                            // Keep text nodes only if they contain non-whitespace characters.
                            c.text().map_or(false, |t| !t.trim().is_empty())
                        } else {
                            // Filter out comments, PIs, etc.
                            false
                        }
                    })
                    .map(XmlNode::Node),
            ),
            XmlNode::Attribute { .. } => Box::new(std::iter::empty()),
        }
    }

    fn parent(&self) -> Option<Self> {
        match self {
            XmlNode::Node(n) => n.parent().map(XmlNode::Node),
            XmlNode::Attribute { parent, .. } => Some(XmlNode::Node(*parent)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_XML: &str = r#"<root id="rootId" xmlns:ns="http://example.com/ns" ns:attr="val">
        <child1>Hello</child1>
        <child2 type="a">
            <grandchild/>
        </child2>
        <!-- comment -->
        <child1> World</child1>
    </root>"#;

    #[test]
    fn test_parse_and_get_root() {
        let doc = XmlDocument::parse(TEST_XML).unwrap();
        let root = doc.root_node();
        assert_eq!(root.node_type(), NodeType::Root);
        let doc_element = root.children().find(|n| n.node_type() == NodeType::Element).unwrap();
        assert_eq!(doc_element.name().unwrap().local_part, "root");
    }

    #[test]
    fn test_node_navigation() {
        let doc = XmlDocument::parse(TEST_XML).unwrap();
        let root = doc.root_node();
        let doc_element = root.children().find(|n| n.node_type() == NodeType::Element).unwrap();


        let children: Vec<_> = doc_element.children().filter(|n| n.node_type() == NodeType::Element).collect();
        // After filtering out whitespace-only text nodes, we have child1, child2, child1.
        assert_eq!(children.len(), 3);

        let child1_first = children[0];
        assert_eq!(child1_first.name().unwrap().local_part, "child1");
        assert_eq!(child1_first.string_value(), "Hello");

        let child2 = children[1];
        assert_eq!(child2.name().unwrap().local_part, "child2");
        assert_eq!(child2.string_value().trim(), "");
        assert_eq!(child2.parent().unwrap(), doc_element);

        let grandchild = child2.children().filter(|c| c.node_type() == NodeType::Element).next().unwrap();
        assert_eq!(grandchild.name().unwrap().local_part, "grandchild");
        assert_eq!(grandchild.parent().unwrap(), child2);
    }
}