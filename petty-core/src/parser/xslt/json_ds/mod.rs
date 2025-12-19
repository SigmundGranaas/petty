// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/json_ds/mod.rs
//! An implementation of the `DataSourceNode` trait for a `serde_json::Value`.
//! It transforms the JSON into an in-memory "Virtual DOM" that can be navigated
//! by the XPath engine as if it were an XML document.

use crate::parser::xslt::datasource::{DataSourceNode, NodeType, QName};
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;

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

    // REMOVED the hardcoded singularize function

    fn build(&mut self, json_value: &'a Value) {
        // Create the top-level root node (ID 0)
        self.nodes.push(VNodeData {
            node_type: NodeType::Root,
            name: None,
            value: "".to_string(), // Root value is calculated on-demand
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
    }


    fn build_recursive(&mut self, val: &'a Value, parent_id: usize, name: &'a str) -> usize {
        match val {
            Value::Object(obj) => {
                let current_id = self.nodes.len();
                self.parent_map.insert(current_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element,
                    name: Some(QName { prefix: None, local_part: name }),
                    value: String::new(), // Value is calculated on-demand
                    children: Vec::new(),
                    attributes: Vec::new(),
                });

                // Collect keys and sort them alphabetically to ensure deterministic order
                let mut sorted_keys: Vec<_> = obj.keys().map(|k| k.as_str()).collect();
                sorted_keys.sort_unstable(); // Use unstable sort for efficiency

                for key_str in sorted_keys {
                    let child_val = &obj[key_str];

                    if let Some(attr_name) = key_str.strip_prefix('@') {
                        // Handle attributes (prefixed with '@')
                        let attr_id = self.build_attribute(child_val, current_id, attr_name);
                        self.nodes[current_id].attributes.push(attr_id);
                    } else if let Value::Array(arr) = child_val {
                        // Handle nested arrays: create a container element for the array itself
                        let container_id = self.nodes.len();
                        self.parent_map.insert(container_id, current_id);
                        self.nodes.push(VNodeData {
                            node_type: NodeType::Element,
                            name: Some(QName { prefix: None, local_part: key_str }),
                            value: String::new(), // Container value is calculated on-demand
                            children: vec![],
                            attributes: vec![],
                        });
                        self.nodes[current_id].children.push(container_id);

                        // Create children for each item within the container.
                        // Use "item" as the standard name for array elements.
                        let item_name = "item";
                        for item in arr {
                            if !item.is_null() {
                                let item_id = self.build_recursive(item, container_id, item_name);
                                self.nodes[container_id].children.push(item_id);
                            }
                        }

                    } else if !child_val.is_null() {
                        // Handle nested objects or primitives (non-null)
                        let child_id = self.build_recursive(child_val, current_id, key_str);
                        self.nodes[current_id].children.push(child_id);
                    }
                }
                current_id
            }
            Value::Array(arr) => {
                // This case handles when an array is NOT an object value (e.g., top-level).
                // Create a container element.
                let current_id = self.nodes.len();
                self.parent_map.insert(current_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element,
                    name: Some(QName { prefix: None, local_part: name }),
                    value: String::new(), // Value calculated on-demand
                    children: Vec::new(),
                    attributes: vec![],
                });

                // Use "item" as the standard name for array elements
                let item_name = "item";
                for item in arr {
                    if !item.is_null() {
                        let child_id = self.build_recursive(item, current_id, item_name);
                        self.nodes[current_id].children.push(child_id);
                    }
                }
                current_id
            }
            Value::Null => {
                // Create an empty element for null values
                let current_id = self.nodes.len();
                self.parent_map.insert(current_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element, name: Some(QName { prefix: None, local_part: name }),
                    value: String::new(), children: vec![], attributes: vec![],
                });
                current_id
            }
            primitive => {
                // Handle primitive values (String, Number, Bool)
                let owned_value_str = match primitive {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => unreachable!(), // Should only be string, number, or bool here
                };

                // Create the element node that represents the primitive key-value pair
                let element_id = self.nodes.len();
                self.parent_map.insert(element_id, parent_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Element,
                    name: Some(QName { prefix: None, local_part: name }),
                    value: owned_value_str.clone(), // Element's value is the primitive's value
                    children: vec![element_id + 1], // It will have one text node child
                    attributes: vec![],
                });

                // Create the text node child containing the primitive's value
                let text_id = self.nodes.len();
                self.parent_map.insert(text_id, element_id);
                self.nodes.push(VNodeData {
                    node_type: NodeType::Text, name: None,
                    value: owned_value_str, // Text node's value is also the primitive's value
                    children: vec![], attributes: vec![],
                });
                element_id // Return the ID of the element node
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
            _ => String::new(), // Attributes from non-primitives become empty string
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

impl<'a> PartialOrd for JsonVNode<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<'a> Ord for JsonVNode<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<'a> Hash for JsonVNode<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) { self.id.hash(state); }
}

impl<'a> DataSourceNode<'a> for JsonVNode<'a> {
    fn node_type(&self) -> NodeType { self.tree.nodes.get(self.id).map_or(NodeType::Text, |n| n.node_type) } // Default to Text if ID out of bounds
    fn name(&self) -> Option<QName<'a>> { self.tree.nodes.get(self.id).and_then(|n| n.name) }

    fn string_value(&self) -> String {
        match self.node_type() {
            NodeType::Attribute | NodeType::Text => {
                // The value is pre-stored for these simple node types.
                self.tree.nodes.get(self.id).map_or(String::new(), |n| n.value.clone())
            }
            NodeType::Element | NodeType::Root => {
                // Compute on-demand for elements and root by concatenating all descendant text nodes.
                let mut s = String::new();
                let mut stack: Vec<JsonVNode> = self.children().collect();
                stack.reverse(); // Reverse to process in document order using pop()

                while let Some(node) = stack.pop() {
                    match node.node_type() {
                        NodeType::Text => {
                            if let Some(node_data) = node.tree.nodes.get(node.id) {
                                s.push_str(&node_data.value);
                            }
                        }
                        NodeType::Element => {
                            // Add children to the stack to continue the depth-first traversal.
                            let mut children: Vec<_> = node.children().collect();
                            children.reverse();
                            stack.extend(children);
                        }
                        _ => {} // Ignore attributes, etc.
                    }
                }
                s
            }
            NodeType::Comment | NodeType::ProcessingInstruction => {
                // The JSON VDOM cannot produce these node types.
                unreachable!();
            }
        }
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        let tree = self.tree; // Re-borrow to satisfy lifetime checker
        let attribute_ids = self.tree.nodes.get(self.id).map_or(vec![], |n| n.attributes.clone());
        Box::new(attribute_ids.into_iter().map(move |id| JsonVNode { id, tree }))
    }

    fn children(&self) -> Box<dyn Iterator<Item = Self> + 'a> {
        let tree = self.tree; // Re-borrow
        let children_ids = self.tree.nodes.get(self.id).map_or(vec![], |n| n.children.clone());
        Box::new(children_ids.into_iter().filter_map(move |id| {
            // Ensure child ID is valid before creating the node
            if id < tree.nodes.len() { Some(JsonVNode { id, tree }) } else { None }
        }))
    }

    fn parent(&self) -> Option<Self> { self.tree.parent_map.get(&self.id).map(|&pid| JsonVNode { id: pid, tree: self.tree }) }
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_vdom_structure_and_values() {
        let data = json!({
            "user": {
                "@id": "u123",
                "name": "John Doe", // Should come before orders alphabetically
                "orders": [
                    { "amount": 100, "id": "o1" }, // amount before id
                    { "id": "o2", "amount": 200 }  // id before amount
                ]
            }
        });

        let doc = JsonVDocument::new(&data);
        let root = doc.root_node();
        assert_eq!(root.node_type(), NodeType::Root);

        // Expected string value based on alphabetical order:
        // user -> name -> orders -> item -> amount -> id -> item -> amount -> id
        let expected_string_value = "John Doe100o1200o2";
        assert_eq!(root.string_value(), expected_string_value, "Root string value mismatch");


        let doc_element: JsonVNode<'_> = root.children().next().unwrap();
        assert_eq!(doc_element.name().unwrap().local_part, "user");
        assert_eq!(doc_element.string_value(), expected_string_value, "User element string value mismatch");


        let mut attrs: Vec<_> = doc_element.attributes().collect();
        assert_eq!(attrs.len(), 1);
        let id_attr = attrs.remove(0);
        assert_eq!(id_attr.node_type(), NodeType::Attribute);
        assert_eq!(id_attr.name().unwrap().local_part, "id");
        assert_eq!(id_attr.string_value(), "u123");
        assert_eq!(id_attr.parent().unwrap(), doc_element);

        // Children of 'user' should be 'name' and 'orders' (alphabetical).
        let user_children: Vec<_> = doc_element.children().collect();
        assert_eq!(user_children.len(), 2);
        assert_eq!(user_children[0].name().unwrap().local_part, "name"); // name first
        assert_eq!(user_children[1].name().unwrap().local_part, "orders"); // orders second

        let name_node = &user_children[0];
        assert_eq!(name_node.string_value(), "John Doe");
        assert_eq!(name_node.children().next().unwrap().node_type(), NodeType::Text);


        let orders_container = &user_children[1];
        // Expected container value: amount -> id -> amount -> id
        assert_eq!(orders_container.string_value(), "100o1200o2", "Orders container string value mismatch");


        // The children of the <orders> container are the <item> nodes (standard name).
        let order_items: Vec<_> = orders_container.children().collect();
        assert_eq!(order_items.len(), 2);
        assert_eq!(order_items[0].name().unwrap().local_part, "item"); // Standard name
        assert_eq!(order_items[0].string_value(), "100o1", "First order item string value mismatch"); // amount then id
        assert_eq!(order_items[1].name().unwrap().local_part, "item"); // Standard name
        assert_eq!(order_items[1].string_value(), "200o2", "Second order item string value mismatch"); // amount then id


        // Check children of first order item (should be amount then id)
        let order1_children: Vec<_> = order_items[0].children().collect();
        assert_eq!(order1_children.len(), 2);
        assert_eq!(order1_children[0].name().unwrap().local_part, "amount");
        assert_eq!(order1_children[0].string_value(), "100");
        assert_eq!(order1_children[1].name().unwrap().local_part, "id");
        assert_eq!(order1_children[1].string_value(), "o1");


        // Check children of second order item (should be amount then id)
        let order2_children: Vec<_> = order_items[1].children().collect();
        assert_eq!(order2_children.len(), 2);
        assert_eq!(order2_children[0].name().unwrap().local_part, "amount");
        assert_eq!(order2_children[0].string_value(), "200");
        assert_eq!(order2_children[1].name().unwrap().local_part, "id");
        assert_eq!(order2_children[1].string_value(), "o2");
    }

    #[test]
    fn test_json_vdom_string_value_simple() {
        let data = json!({
            "para": {
                "line2": "World", // Reversed order
                "line1": "Hello"
            }
        });

        let doc = JsonVDocument::new(&data);
        let root = doc.root_node();
        let para_node = root.children().next().unwrap();

        // Expect alphabetical concatenation: Hello then World
        assert_eq!(para_node.string_value(), "HelloWorld");
        assert_eq!(root.string_value(), "HelloWorld");
    }

    #[test]
    fn test_json_vdom_primitive_array() {
        let data = json!({
            "tags": ["rust", "xslt", "json"]
        });
        let doc = JsonVDocument::new(&data);
        let root = doc.root_node(); // Root node
        let tags_node = root.children().next().unwrap(); // <tags> element
        assert_eq!(tags_node.name().unwrap().local_part, "tags");
        assert_eq!(tags_node.string_value(), "rustxsltjson");

        let items: Vec<_> = tags_node.children().collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].name().unwrap().local_part, "item"); // Standard name
        assert_eq!(items[0].string_value(), "rust");
        assert_eq!(items[1].string_value(), "xslt");
        assert_eq!(items[2].string_value(), "json");

    }
}