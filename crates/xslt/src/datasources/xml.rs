// XML datasource implementation using roxmltree
use petty_xpath1::{DataSourceNode, NodeType, QName};
use roxmltree::Node;
use std::hash::{Hash, Hasher};

/// Wrapper around roxmltree::Document providing data source capabilities
pub struct XmlDocument<'input> {
    doc: roxmltree::Document<'input>,
}

impl<'input> XmlDocument<'input> {
    pub fn parse(text: &'input str) -> Result<Self, roxmltree::Error> {
        let doc = roxmltree::Document::parse(text)?;
        Ok(Self { doc })
    }

    pub fn root_node(&self) -> XmlNode<'_, 'input> {
        XmlNode::Element(self.doc.root())
    }
}

/// Represents either an element/text node or an attribute in the XML tree.
/// Attributes need special handling because roxmltree treats them as data on elements,
/// not as navigable nodes in the tree.
#[derive(Debug, Clone, Copy)]
pub enum XmlNode<'a, 'input> {
    /// A regular node (element, text, comment, etc.)
    Element(Node<'a, 'input>),
    /// An attribute, represented by its parent element and the attribute index
    Attribute {
        parent: Node<'a, 'input>,
        index: usize,
    },
}

impl<'a, 'input> XmlNode<'a, 'input> {
    pub fn new(node: Node<'a, 'input>) -> Self {
        XmlNode::Element(node)
    }

    pub fn inner(&self) -> Option<Node<'a, 'input>> {
        match self {
            XmlNode::Element(node) => Some(*node),
            XmlNode::Attribute { .. } => None,
        }
    }
}

impl<'a, 'input> PartialEq for XmlNode<'a, 'input> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (XmlNode::Element(a), XmlNode::Element(b)) => a.id() == b.id(),
            (
                XmlNode::Attribute {
                    parent: p1,
                    index: i1,
                },
                XmlNode::Attribute {
                    parent: p2,
                    index: i2,
                },
            ) => p1.id() == p2.id() && i1 == i2,
            _ => false,
        }
    }
}

impl<'a, 'input> Eq for XmlNode<'a, 'input> {}

impl<'a, 'input> PartialOrd for XmlNode<'a, 'input> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a, 'input> Ord for XmlNode<'a, 'input> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (XmlNode::Element(a), XmlNode::Element(b)) => a.id().get().cmp(&b.id().get()),
            (
                XmlNode::Attribute {
                    parent: p1,
                    index: i1,
                },
                XmlNode::Attribute {
                    parent: p2,
                    index: i2,
                },
            ) => match p1.id().get().cmp(&p2.id().get()) {
                std::cmp::Ordering::Equal => i1.cmp(i2),
                other => other,
            },
            // Elements come before their attributes in document order
            (XmlNode::Element(e), XmlNode::Attribute { parent, .. }) => {
                if e.id() == parent.id() {
                    std::cmp::Ordering::Less
                } else {
                    e.id().get().cmp(&parent.id().get())
                }
            }
            (XmlNode::Attribute { parent, .. }, XmlNode::Element(e)) => {
                if parent.id() == e.id() {
                    std::cmp::Ordering::Greater
                } else {
                    parent.id().get().cmp(&e.id().get())
                }
            }
        }
    }
}

impl<'a, 'input> Hash for XmlNode<'a, 'input> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            XmlNode::Element(node) => {
                0u8.hash(state);
                node.id().hash(state);
            }
            XmlNode::Attribute { parent, index } => {
                1u8.hash(state);
                parent.id().hash(state);
                index.hash(state);
            }
        }
    }
}

impl<'a> DataSourceNode<'a> for XmlNode<'a, 'a> {
    fn node_type(&self) -> NodeType {
        match self {
            XmlNode::Element(node) => {
                if node.is_root() {
                    NodeType::Root
                } else if node.is_element() {
                    NodeType::Element
                } else if node.is_text() {
                    NodeType::Text
                } else if node.is_comment() {
                    NodeType::Comment
                } else if node.is_pi() {
                    NodeType::ProcessingInstruction
                } else {
                    NodeType::Element
                }
            }
            XmlNode::Attribute { .. } => NodeType::Attribute,
        }
    }

    fn name(&self) -> Option<QName<'a>> {
        match self {
            XmlNode::Element(node) => {
                if let Some(tag) = (!node.tag_name().name().is_empty()).then(|| node.tag_name()) {
                    Some(QName {
                        prefix: tag.namespace().and(None), // roxmltree doesn't expose prefix directly
                        local_part: tag.name(),
                    })
                } else if node.is_pi() {
                    node.pi().map(|pi| QName {
                        prefix: None,
                        local_part: pi.target,
                    })
                } else {
                    None
                }
            }
            XmlNode::Attribute { parent, index } => {
                parent.attributes().nth(*index).map(|attr| {
                    // Check for xml: prefix by looking at the namespace
                    let prefix = if attr.namespace() == Some("http://www.w3.org/XML/1998/namespace")
                    {
                        Some("xml")
                    } else {
                        None
                    };
                    QName {
                        prefix,
                        local_part: attr.name(),
                    }
                })
            }
        }
    }

    fn string_value(&self) -> String {
        match self {
            XmlNode::Element(node) => {
                if node.is_text() {
                    node.text().unwrap_or("").to_string()
                } else if node.is_element() || node.is_root() {
                    node.descendants()
                        .filter(|n| n.is_text())
                        .filter_map(|n| n.text())
                        .collect::<Vec<_>>()
                        .join("")
                } else if node.is_comment() {
                    node.text().unwrap_or("").to_string()
                } else if node.is_pi() {
                    node.pi()
                        .map(|pi| pi.value.unwrap_or(""))
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                }
            }
            XmlNode::Attribute { parent, index } => parent
                .attributes()
                .nth(*index)
                .map(|attr| attr.value().to_string())
                .unwrap_or_default(),
        }
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        match self {
            XmlNode::Element(node) => {
                let parent = *node;
                let attr_count = node.attributes().len();
                Box::new((0..attr_count).map(move |index| XmlNode::Attribute { parent, index }))
            }
            XmlNode::Attribute { .. } => {
                // Attributes don't have attributes
                Box::new(std::iter::empty())
            }
        }
    }

    fn children(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        match self {
            XmlNode::Element(node) => Box::new(node.children().map(XmlNode::Element)),
            XmlNode::Attribute { .. } => {
                // Attributes don't have children
                Box::new(std::iter::empty())
            }
        }
    }

    fn parent(&self) -> Option<Self> {
        match self {
            XmlNode::Element(node) => node.parent().map(XmlNode::Element),
            XmlNode::Attribute { parent, .. } => Some(XmlNode::Element(*parent)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_node_attributes() {
        let xml = r#"<root><item id="123" status="active">Text</item></root>"#;
        let doc = XmlDocument::parse(xml).unwrap();
        let root = doc.root_node();

        // Navigate to <item>
        let item = root
            .children()
            .find(|n| n.name().map(|q| q.local_part == "root").unwrap_or(false))
            .unwrap();
        let item = item
            .children()
            .find(|n| n.name().map(|q| q.local_part == "item").unwrap_or(false))
            .unwrap();

        // Check attributes
        let attrs: Vec<_> = item.attributes().collect();
        assert_eq!(attrs.len(), 2);

        // Check first attribute
        assert_eq!(attrs[0].node_type(), NodeType::Attribute);
        assert_eq!(attrs[0].name().unwrap().local_part, "id");
        assert_eq!(attrs[0].string_value(), "123");

        // Check second attribute
        assert_eq!(attrs[1].name().unwrap().local_part, "status");
        assert_eq!(attrs[1].string_value(), "active");

        // Check attribute parent
        assert_eq!(attrs[0].parent(), Some(item));
    }

    #[test]
    fn test_xml_node_navigation() {
        let xml = r#"<data><users><user status="active">Alice</user></users></data>"#;
        let doc = XmlDocument::parse(xml).unwrap();
        let root = doc.root_node();

        // Navigate to <user>
        let data = root
            .children()
            .find(|n| n.name().map(|q| q.local_part == "data").unwrap_or(false))
            .unwrap();
        let users = data
            .children()
            .find(|n| n.name().map(|q| q.local_part == "users").unwrap_or(false))
            .unwrap();
        let user = users
            .children()
            .find(|n| n.name().map(|q| q.local_part == "user").unwrap_or(false))
            .unwrap();

        // Check the status attribute
        let status_attr = user
            .attributes()
            .find(|a| a.name().map(|q| q.local_part == "status").unwrap_or(false))
            .unwrap();
        assert_eq!(status_attr.string_value(), "active");
    }
}
