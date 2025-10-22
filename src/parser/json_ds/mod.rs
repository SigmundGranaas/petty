// FILE: src/parser/json_ds/mod.rs
//! An implementation of the `DataSourceNode` trait for a `serde_json::Value`.
//! It transforms the JSON into an in-memory "Virtual DOM" that can be navigated
//! by the XPath engine as if it were an XML document.

use crate::parser::datasource::{DataSourceNode, NodeType, QName};
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// --- VDOM Data Structures ---

#[derive(Debug, Clone)]
struct VNodeData<'a> {
    node_type: NodeType,
    name: Option<QName<'a>>,
    value: String,
    children: Vec<usize>,
    attributes: Vec<usize>,
}

#[derive(Debug)]
pub struct JsonVDocument<'a> {
    nodes: Vec<VNodeData<'a>>,
    parent_map: HashMap<usize, usize>,
    _lifetime_marker: std::marker::PhantomData<&'a Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct JsonVNode<'a> {
    id: usize,
    tree: &'a JsonVDocument<'a>,
}

// --- VDOM Builder ---

impl<'a> JsonVDocument<'a> {
    /// Parses a `serde_json::Value` into a navigable VDOM document.
    pub fn new(json_value: &'a Value) -> Self {
        let mut doc = JsonVDocument {
            nodes: Vec::new(),
            parent_map: HashMap::new(),
            _lifetime_marker: std::marker::PhantomData,
        };
        doc.build(json_value);
        doc
    }

    /// Returns the root node of the VDOM.
    pub fn root_node(&'a self) -> JsonVNode<'a> {
        JsonVNode { id: 0, tree: self }
    }

    /// A simple, non-exhaustive singularization helper for common resume keys.
    fn singularize(plural: &str) -> &str {
        match plural {
            "experience" => "job",
            "education" => "entry",
            "skills" => "skill",
            "projects" => "project",
            "responsibilities" => "item",
            _ => "item", // default fallback
        }
    }

    fn build(&mut self, json_value: &'a Value) {
        // Create the top-level root node (ID 0)
        self.nodes.push(VNodeData {
            node_type: NodeType::Root,
            name: None,
            value: "".to_string(),
            children: vec![],
            attributes: vec![],
        });

        // Determine the name and content for the document element.
        // If the top-level JSON is an object with a single key, that key becomes the document element.
        // Otherwise, a synthetic "root" element is created.
        let (doc_element_name, doc_element_content) = match json_value {
            Value::Object(obj) if obj.len() == 1 => {
                let (key, value) = obj.iter().next().unwrap();
                (key.as_str(), value)
            }
            _ => ("root", json_value),
        };

        // Start the recursive build from the document element
        let doc_element_id = self.build_recursive(doc_element_content, 0, doc_element_name);
        self.nodes[0].children.push(doc_element_id);

        // Post-process the root's string value.
        let string_val = {
            let root_node_copy = JsonVNode { id: 0, tree: self };
            root_node_copy.calculate_string_value()
        };
        if let Some(node_data) = self.nodes.get_mut(0) {
            node_data.value = string_val;
        }
    }

    fn build_recursive(&mut self, val: &'a Value, parent_id: usize, name: &'a str) -> usize {
        match val {
            Value::Object(obj) => {
                let current_id = self.nodes.len();
                self.parent_map.insert(current_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element,
                    name: Some(QName { prefix: None, local_part: name }),
                    value: String::new(),
                    children: Vec::new(),
                    attributes: Vec::new(),
                });

                for (key, child_val) in obj {
                    if let Some(attr_name) = key.strip_prefix('@') {
                        let attr_id = self.build_attribute(child_val, current_id, attr_name);
                        self.nodes[current_id].attributes.push(attr_id);
                    } else if let Value::Array(arr) = child_val {
                        // **BUG FIX STARTS HERE**
                        // Create a single container element for the array.
                        let container_id = self.nodes.len();
                        self.parent_map.insert(container_id, current_id);
                        self.nodes.push(VNodeData {
                            node_type: NodeType::Element,
                            name: Some(QName { prefix: None, local_part: key }),
                            value: String::new(), children: vec![], attributes: vec![],
                        });
                        self.nodes[current_id].children.push(container_id);

                        // Now, create children for each item within the container.
                        let item_name = Self::singularize(key);
                        for item in arr {
                            if !item.is_null() {
                                let item_id = self.build_recursive(item, container_id, item_name);
                                self.nodes[container_id].children.push(item_id);
                            }
                        }
                        // **BUG FIX ENDS HERE**
                    } else if !child_val.is_null() {
                        let child_id = self.build_recursive(child_val, current_id, key);
                        self.nodes[current_id].children.push(child_id);
                    }
                }
                current_id
            }
            Value::Array(arr) => {
                // This case handles when an array is NOT an object value (e.g., top-level).
                // We create a container element.
                let current_id = self.nodes.len();
                self.parent_map.insert(current_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element,
                    name: Some(QName { prefix: None, local_part: name }),
                    value: String::new(),
                    children: Vec::new(),
                    attributes: vec![],
                });

                for item in arr {
                    if !item.is_null() {
                        let child_id = self.build_recursive(item, current_id, "item");
                        self.nodes[current_id].children.push(child_id);
                    }
                }
                current_id
            }
            Value::Null => {
                let current_id = self.nodes.len();
                self.parent_map.insert(current_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element, name: Some(QName { prefix: None, local_part: name }),
                    value: String::new(), children: vec![], attributes: vec![],
                });
                current_id
            }
            primitive => {
                let owned_value_str = match primitive {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => unreachable!(),
                };

                let element_id = self.nodes.len();
                self.parent_map.insert(element_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element,
                    name: Some(QName { prefix: None, local_part: name }),
                    value: owned_value_str.clone(),
                    children: vec![element_id + 1],
                    attributes: vec![],
                });

                let text_id = self.nodes.len();
                self.parent_map.insert(text_id, element_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Text, name: None,
                    value: owned_value_str, children: vec![], attributes: vec![],
                });
                element_id
            }
        }
    }

    fn build_attribute(&mut self, val: &'a Value, parent_id: usize, name: &'a str) -> usize {
        let attr_id = self.nodes.len();
        self.parent_map.insert(attr_id, parent_id);
        let attr_value = match val {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        };
        self.nodes.push(VNodeData {
            node_type: NodeType::Attribute,
            name: Some(QName { prefix: None, local_part: name }),
            value: attr_value, children: vec![], attributes: vec![],
        });
        attr_id
    }
}

// --- DataSourceNode Trait Implementation ---

impl<'a> PartialEq for JsonVNode<'a> {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}
impl<'a> Eq for JsonVNode<'a> {}

impl<'a> Hash for JsonVNode<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) { self.id.hash(state); }
}

impl<'a> DataSourceNode<'a> for JsonVNode<'a> {
    fn node_type(&self) -> NodeType { self.tree.nodes.get(self.id).map_or(NodeType::Text, |n| n.node_type) }
    fn name(&self) -> Option<QName<'a>> { self.tree.nodes.get(self.id).and_then(|n| n.name) }
    fn string_value(&self) -> String {
        if self.node_type() == NodeType::Element || self.node_type() == NodeType::Root {
            self.calculate_string_value()
        } else {
            self.tree.nodes.get(self.id).map_or(String::new(), |n| n.value.clone())
        }
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        let tree = self.tree;
        let attribute_ids = self.tree.nodes.get(self.id).map_or(vec![], |n| n.attributes.clone());
        Box::new(attribute_ids.into_iter().map(move |id| JsonVNode { id, tree }))
    }
    fn children(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        let tree = self.tree;
        let children_ids = self.tree.nodes.get(self.id).map_or(vec![], |n| n.children.clone());
        Box::new(children_ids.into_iter().filter_map(move |id| {
            if id < tree.nodes.len() { Some(JsonVNode { id, tree }) } else { None }
        }))
    }
    fn parent(&self) -> Option<Self> { self.tree.parent_map.get(&self.id).map(|&pid| JsonVNode { id: pid, tree: self.tree }) }
}

impl<'a> JsonVNode<'a> {
    fn calculate_string_value(&self) -> String {
        if self.node_type() == NodeType::Text {
            return self.tree.nodes.get(self.id).map_or(String::new(), |n| n.value.clone());
        }
        let mut s = String::new();
        let mut stack: Vec<JsonVNode> = self.children().collect();
        stack.reverse();
        while let Some(node) = stack.pop() {
            if node.node_type() == NodeType::Text {
                if let Some(node_data) = node.tree.nodes.get(node.id) {
                    s.push_str(&node_data.value);
                }
            } else {
                let mut children: Vec<_> = node.children().collect();
                children.reverse();
                stack.extend(children);
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_vdom_structure() {
        let data = json!({
            "user": {
                "@id": "u123",
                "name": "John Doe",
                "orders": [
                    { "id": "o1", "amount": 100 },
                    { "id": "o2", "amount": 200 }
                ]
            }
        });

        let doc = JsonVDocument::new(&data);
        let root = doc.root_node();
        assert_eq!(root.node_type(), NodeType::Root);

        let doc_element: JsonVNode<'_> = root.children().next().unwrap();
        assert_eq!(doc_element.name().unwrap().local_part, "user");

        let mut attrs: Vec<_> = doc_element.attributes().collect();
        assert_eq!(attrs.len(), 1);
        let id_attr = attrs.remove(0);
        assert_eq!(id_attr.node_type(), NodeType::Attribute);
        assert_eq!(id_attr.name().unwrap().local_part, "id");
        assert_eq!(id_attr.string_value(), "u123");
        assert_eq!(id_attr.parent().unwrap(), doc_element);

        // Children of 'user' should be 'name' and the 'orders' container.
        let user_children: Vec<_> = doc_element.children().collect();
        assert_eq!(user_children.len(), 2);

        let orders_container = user_children
            .iter()
            .find(|n| n.name().map_or(false, |q| q.local_part == "orders"))
            .expect("Should find an <orders> container node");

        // The children of the <orders> container are the <item> nodes.
        let order_items: Vec<_> = orders_container.children().collect();
        assert_eq!(order_items.len(), 2);
        assert_eq!(order_items[0].name().unwrap().local_part, "item");

        let order1_id_node = order_items[0]
            .children()
            .find(|c| c.name().unwrap().local_part == "id")
            .unwrap();
        assert_eq!(order1_id_node.string_value(), "o1");

        let order2_amount_node = order_items[1]
            .children()
            .find(|c| c.name().unwrap().local_part == "amount")
            .unwrap();
        assert_eq!(order2_amount_node.string_value(), "200");
    }

    #[test]
    fn test_json_vdom_string_value() {
        let data = json!({
            "para": {
                "line1": "Hello",
                "line2": "World"
            }
        });

        let doc = JsonVDocument::new(&data);
        let root = doc.root_node();
        let para_node = root.children().next().unwrap();

        assert_eq!(para_node.string_value(), "HelloWorld");
    }
}