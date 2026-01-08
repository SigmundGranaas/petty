use super::event_model::{AncestorInfo, Attribute, QName};
use petty_xpath31::types::XdmValue;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StreamedNodeKind {
    Document,
    Element,
    Attribute,
    Text,
    Comment,
    ProcessingInstruction,
}

#[derive(Debug, Clone)]
pub struct StreamedNode {
    pub kind: StreamedNodeKind,
    pub name: Option<QName>,
    pub value: Option<String>,
    pub attributes: Vec<Attribute>,
    pub depth: usize,
    pub position: usize,
}

impl StreamedNode {
    pub fn document() -> Self {
        Self {
            kind: StreamedNodeKind::Document,
            name: None,
            value: None,
            attributes: vec![],
            depth: 0,
            position: 0,
        }
    }

    pub fn element(name: QName, attributes: Vec<Attribute>, depth: usize, position: usize) -> Self {
        Self {
            kind: StreamedNodeKind::Element,
            name: Some(name),
            value: None,
            attributes,
            depth,
            position,
        }
    }

    pub fn text(content: String, depth: usize, position: usize) -> Self {
        Self {
            kind: StreamedNodeKind::Text,
            name: None,
            value: Some(content),
            attributes: vec![],
            depth,
            position,
        }
    }

    pub fn attribute(name: QName, value: String, depth: usize) -> Self {
        Self {
            kind: StreamedNodeKind::Attribute,
            name: Some(name),
            value: Some(value),
            attributes: vec![],
            depth,
            position: 0,
        }
    }

    pub fn comment(content: String, depth: usize, position: usize) -> Self {
        Self {
            kind: StreamedNodeKind::Comment,
            name: None,
            value: Some(content),
            attributes: vec![],
            depth,
            position,
        }
    }

    pub fn processing_instruction(
        target: String,
        data: String,
        depth: usize,
        position: usize,
    ) -> Self {
        Self {
            kind: StreamedNodeKind::ProcessingInstruction,
            name: Some(QName::new(target)),
            value: Some(data),
            attributes: vec![],
            depth,
            position,
        }
    }

    pub fn is_element(&self) -> bool {
        self.kind == StreamedNodeKind::Element
    }

    pub fn is_attribute(&self) -> bool {
        self.kind == StreamedNodeKind::Attribute
    }

    pub fn is_text(&self) -> bool {
        self.kind == StreamedNodeKind::Text
    }

    pub fn local_name(&self) -> Option<&str> {
        self.name.as_ref().map(|n| n.local_name.as_str())
    }

    pub fn string_value(&self) -> String {
        match &self.kind {
            StreamedNodeKind::Text
            | StreamedNodeKind::Comment
            | StreamedNodeKind::Attribute
            | StreamedNodeKind::ProcessingInstruction => self.value.clone().unwrap_or_default(),
            StreamedNodeKind::Element | StreamedNodeKind::Document => String::new(),
        }
    }
}

pub struct StreamedContext {
    ancestors: Vec<AncestorInfo>,
    current: Option<StreamedNode>,
    accumulators: HashMap<String, XdmValue<StreamedNode>>,
    depth: usize,
    position_stack: Vec<usize>,
}

impl StreamedContext {
    pub fn new() -> Self {
        Self {
            ancestors: Vec::new(),
            current: None,
            accumulators: HashMap::new(),
            depth: 0,
            position_stack: vec![0],
        }
    }

    pub fn current_node(&self) -> Option<&StreamedNode> {
        self.current.as_ref()
    }

    pub fn set_current(&mut self, node: StreamedNode) {
        self.current = Some(node);
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn current_position(&self) -> usize {
        self.current.as_ref().map(|n| n.position).unwrap_or(0)
    }

    pub fn push_element(&mut self, name: QName, attributes: Vec<Attribute>) {
        if let Some(current) = &self.current
            && current.is_element()
        {
            self.ancestors.push(AncestorInfo {
                name: current.name.clone().unwrap_or_else(|| QName::new("")),
                attributes: current.attributes.clone(),
                position: current.position,
            });
        }

        self.depth += 1;
        let position = *self.position_stack.last().unwrap_or(&0) + 1;
        *self.position_stack.last_mut().unwrap_or(&mut 0) = position;
        self.position_stack.push(0);

        self.current = Some(StreamedNode::element(
            name, attributes, self.depth, position,
        ));
    }

    pub fn pop_element(&mut self) {
        self.position_stack.pop();
        self.depth = self.depth.saturating_sub(1);

        if let Some(ancestor) = self.ancestors.pop() {
            self.current = Some(StreamedNode::element(
                ancestor.name,
                ancestor.attributes,
                self.depth,
                ancestor.position,
            ));
        } else {
            self.current = Some(StreamedNode::document());
        }
    }

    pub fn process_text(&mut self, content: String) {
        let position = *self.position_stack.last().unwrap_or(&0) + 1;
        *self.position_stack.last_mut().unwrap_or(&mut 0) = position;
        self.current = Some(StreamedNode::text(content, self.depth, position));
    }

    pub fn process_comment(&mut self, content: String) {
        let position = *self.position_stack.last().unwrap_or(&0) + 1;
        *self.position_stack.last_mut().unwrap_or(&mut 0) = position;
        self.current = Some(StreamedNode::comment(content, self.depth, position));
    }

    pub fn process_pi(&mut self, target: String, data: String) {
        let position = *self.position_stack.last().unwrap_or(&0) + 1;
        *self.position_stack.last_mut().unwrap_or(&mut 0) = position;
        self.current = Some(StreamedNode::processing_instruction(
            target, data, self.depth, position,
        ));
    }

    pub fn ancestor_axis(&self) -> impl Iterator<Item = StreamedNode> + '_ {
        self.ancestors
            .iter()
            .rev()
            .map(|a| StreamedNode::element(a.name.clone(), a.attributes.clone(), 0, a.position))
    }

    pub fn attribute_axis(&self) -> impl Iterator<Item = StreamedNode> + '_ {
        self.current.as_ref().into_iter().flat_map(|node| {
            node.attributes.iter().map(|attr| {
                StreamedNode::attribute(attr.name.clone(), attr.value.clone(), node.depth)
            })
        })
    }

    pub fn set_accumulator(&mut self, name: String, value: XdmValue<StreamedNode>) {
        self.accumulators.insert(name, value);
    }

    pub fn get_accumulator(&self, name: &str) -> Option<&XdmValue<StreamedNode>> {
        self.accumulators.get(name)
    }

    pub fn accumulators(&self) -> &HashMap<String, XdmValue<StreamedNode>> {
        &self.accumulators
    }
}

impl Default for StreamedContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streamed_node_element() {
        let node = StreamedNode::element(
            QName::new("div"),
            vec![Attribute {
                name: QName::new("class"),
                value: "container".to_string(),
            }],
            1,
            1,
        );
        assert!(node.is_element());
        assert_eq!(node.local_name(), Some("div"));
        assert_eq!(node.depth, 1);
    }

    #[test]
    fn test_streamed_node_text() {
        let node = StreamedNode::text("Hello, world!".to_string(), 2, 3);
        assert!(node.is_text());
        assert_eq!(node.string_value(), "Hello, world!");
    }

    #[test]
    fn test_streamed_context_navigation() {
        let mut ctx = StreamedContext::new();

        ctx.push_element(QName::new("root"), vec![]);
        assert_eq!(ctx.depth(), 1);

        ctx.push_element(QName::new("child"), vec![]);
        assert_eq!(ctx.depth(), 2);

        let ancestors: Vec<_> = ctx.ancestor_axis().collect();
        assert_eq!(ancestors.len(), 1);
        assert_eq!(ancestors[0].local_name(), Some("root"));

        ctx.pop_element();
        assert_eq!(ctx.depth(), 1);
    }

    #[test]
    fn test_streamed_context_attributes() {
        let mut ctx = StreamedContext::new();
        ctx.push_element(
            QName::new("div"),
            vec![
                Attribute {
                    name: QName::new("id"),
                    value: "main".to_string(),
                },
                Attribute {
                    name: QName::new("class"),
                    value: "container".to_string(),
                },
            ],
        );

        let attrs: Vec<_> = ctx.attribute_axis().collect();
        assert_eq!(attrs.len(), 2);
        assert!(attrs.iter().any(|a| a.local_name() == Some("id")));
        assert!(attrs.iter().any(|a| a.local_name() == Some("class")));
    }

    #[test]
    fn test_streamed_context_position_tracking() {
        let mut ctx = StreamedContext::new();

        ctx.push_element(QName::new("root"), vec![]);
        assert_eq!(ctx.current_position(), 1);

        ctx.push_element(QName::new("first"), vec![]);
        ctx.pop_element();

        ctx.push_element(QName::new("second"), vec![]);
        assert_eq!(ctx.current_position(), 2);
    }

    #[test]
    fn test_accumulator_storage() {
        let mut ctx = StreamedContext::new();
        ctx.set_accumulator("total".to_string(), XdmValue::from_integer(42));

        let value = ctx.get_accumulator("total");
        assert!(value.is_some());
    }
}
