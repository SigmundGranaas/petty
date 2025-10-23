// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/xml/mod.rs
// FILE: src/parser/xml/mod.rs
//! An implementation of the `DataSourceNode` trait for the `roxmltree` crate.
//! This provides a high-performance, compliant XML data source.

use crate::parser::xslt::datasource::{DataSourceNode, NodeType, QName};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;

/// A wrapper around a `roxmltree::Document` that acts as the entry point
/// for processing an XML data source.
///
/// It holds the parsed document in memory and provides access to the root node.
#[derive(Debug)]
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

impl<'a> PartialOrd for XmlNode<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for XmlNode<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Two nodes are compared by their document order.
            (XmlNode::Node(a), XmlNode::Node(b)) => a.cmp(b),
            // An attribute is ordered relative to its parent element.
            (XmlNode::Node(n), XmlNode::Attribute { parent: p, .. }) => {
                // If the node is the parent of the attribute, the node comes first.
                // Otherwise, compare the node to the attribute's parent.
                n.cmp(p).then(Ordering::Less)
            },
            (XmlNode::Attribute { parent: p, .. }, XmlNode::Node(n)) => {
                p.cmp(n).then(Ordering::Greater)
            },
            // Two attributes are ordered by their parent elements first,
            // then by their names (implementation-defined order).
            (XmlNode::Attribute { attr: a, parent: pa }, XmlNode::Attribute { attr: b, parent: pb }) => {
                pa.cmp(pb).then_with(|| a.name().cmp(b.name()))
            }
        }
    }
}

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
            XmlNode::Node(n) if n.is_comment() => NodeType::Comment,
            XmlNode::Node(n) if n.is_pi() => NodeType::ProcessingInstruction,
            XmlNode::Attribute { .. } => NodeType::Attribute,
            _ => unreachable!(),
        }
    }

    fn name(&self) -> Option<QName<'a>> {
        match self {
            XmlNode::Node(n) if n.is_element() => {
                let tag_name = n.tag_name();
                let uri = tag_name.namespace();
                // `lookup_prefix` finds a prefix for a given URI in the scope of the node.
                let prefix = uri.and_then(|u| n.lookup_prefix(u));
                Some(QName {
                    prefix,
                    local_part: tag_name.name(),
                })
            }
            XmlNode::Node(n) if n.is_pi() => {
                let pi = n.pi().unwrap();
                Some(QName {
                    prefix: None,
                    local_part: pi.target,
                })
            }
            XmlNode::Attribute { attr, parent } => {
                let uri = attr.namespace();
                // The 'xml' prefix is implicit and might not be found by lookup_prefix,
                // so we handle it as a special case.
                let prefix = if uri == Some("http://www.w3.org/XML/1998/namespace") {
                    Some("xml")
                } else {
                    uri.and_then(|u| parent.lookup_prefix(u))
                };

                Some(QName {
                    prefix,
                    local_part: attr.name(),
                })
            }
            _ => None,
        }
    }

    fn string_value(&self) -> String {
        match self {
            XmlNode::Attribute { attr, .. } => attr.value().to_string(),
            XmlNode::Node(n) if n.is_text() => n.text().unwrap_or("").to_string(),
            XmlNode::Node(n) if n.is_comment() => n.text().unwrap_or("").to_string(),
            XmlNode::Node(n) if n.is_pi() => n.pi().and_then(|pi| pi.value).unwrap_or("").to_string(),
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
                        if c.is_element() || c.is_comment() || c.is_pi() {
                            true
                        } else if c.is_text() {
                            // Keep text nodes only if they contain non-whitespace characters.
                            c.text().map_or(false, |t| !t.trim().is_empty())
                        } else {
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
        <?pi-target pi-value?>
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


        let children: Vec<_> = doc_element.children().collect();
        // After filtering whitespace-only text, we have child1, child2, comment, pi, child1.
        assert_eq!(children.len(), 5);

        let child1_first = children[0];
        assert_eq!(child1_first.name().unwrap().local_part, "child1");
        assert_eq!(child1_first.string_value(), "Hello");

        let child2 = children[1];
        assert_eq!(child2.name().unwrap().local_part, "child2");
        assert_eq!(child2.string_value().trim(), "");
        assert_eq!(child2.parent().unwrap(), doc_element);

        let comment = children[2];
        assert_eq!(comment.node_type(), NodeType::Comment);
        assert_eq!(comment.string_value(), " comment ");
        assert_eq!(comment.parent().unwrap(), doc_element);

        let pi = children[3];
        assert_eq!(pi.node_type(), NodeType::ProcessingInstruction);
        assert_eq!(pi.name().unwrap().local_part, "pi-target");
        assert_eq!(pi.string_value(), "pi-value");

        let grandchild = child2.children().filter(|c| c.node_type() == NodeType::Element).next().unwrap();
        assert_eq!(grandchild.name().unwrap().local_part, "grandchild");
        assert_eq!(grandchild.parent().unwrap(), child2);
    }

    #[test]
    fn test_document_order_comparison() {
        let doc = XmlDocument::parse(TEST_XML).unwrap();
        let root = doc.root_node();
        let doc_element = root.children().find(|n| n.node_type() == NodeType::Element).unwrap();

        // Extract the inner roxmltree::Node to call descendants()
        let ro_doc_element = if let XmlNode::Node(n) = doc_element { n } else { panic!() };
        let all_descendants: Vec<_> = ro_doc_element.descendants().map(XmlNode::Node).collect();

        let child1_first = all_descendants.iter().find(|n| n.name().map_or(false, |q| q.local_part == "child1") && n.string_value() == "Hello").unwrap();
        let child2 = all_descendants.iter().find(|n| n.name().map_or(false, |q| q.local_part == "child2")).unwrap();
        let grandchild = all_descendants.iter().find(|n| n.name().map_or(false, |q| q.local_part == "grandchild")).unwrap();
        let attr = doc_element.attributes().find(|a| a.name().map_or(false, |q| q.local_part == "id")).unwrap();

        assert!(*child1_first < *child2);
        assert!(*child2 < *grandchild);

        // Attribute ordering
        assert!(doc_element < attr); // Attribute comes after its parent
        assert!(attr < *child1_first); // But before parent's children

        // Test sorting
        let mut nodes_to_sort = vec![*grandchild, root, *child2, doc_element, attr, *child1_first];
        nodes_to_sort.sort();

        let expected_order = vec![root, doc_element, attr, *child1_first, *child2, *grandchild];

        assert_eq!(nodes_to_sort, expected_order);
    }
}