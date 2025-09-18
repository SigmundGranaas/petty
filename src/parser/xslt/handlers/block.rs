// src/parser/xslt/handlers/block.rs
// src/parser/xslt/handlers/block.rs
use super::super::builder::TreeBuilder;
use super::super::util::{
    get_attr_owned_optional, parse_fo_attributes_to_element_style, OwnedAttributes,
};
use crate::error::PipelineError;
use crate::idf::IRNode;
use quick_xml::events::BytesStart;

impl<'a> TreeBuilder<'a> {
    pub(in crate::parser::xslt) fn handle_block_element(
        &mut self,
        e: &BytesStart,
        attributes: &OwnedAttributes,
    ) -> Result<(), PipelineError> {
        let style_name = get_attr_owned_optional(attributes, b"style")?;
        let style_override = parse_fo_attributes_to_element_style(attributes)?;

        let node = match e.name().as_ref() {
            b"list" => IRNode::List {
                style_name,
                style_override,
                children: vec![],
            },
            b"list-item" => IRNode::ListItem {
                style_name,
                style_override,
                children: vec![],
            },
            b"flex-container" => IRNode::FlexContainer {
                style_name,
                style_override,
                children: vec![],
            },
            b"text" => IRNode::Paragraph {
                style_name,
                style_override,
                children: vec![],
            },
            // Default to a generic block container
            _ => IRNode::Block {
                style_name,
                style_override,
                children: vec![],
            },
        };
        self.node_stack.push(node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::idf::IRNode;
    use crate::parser::xslt::handlers::test_helpers::build_fragment;
    use serde_json::json;

    #[test]
    fn test_handle_simple_container() {
        let xml = r#"<container style="main">Some text</container>"#;
        let nodes = build_fragment(xml, &json!({})).unwrap();

        assert_eq!(nodes.len(), 1);
        if let IRNode::Block { style_name, .. } = &nodes[0] {
            assert_eq!(style_name.as_deref(), Some("main"));
        } else {
            panic!("Expected IRNode::Block");
        }
    }

    #[test]
    fn test_handle_paragraph_with_inline_style() {
        let xml = r#"<text font-size="16pt">Hello</text>"#;
        let nodes = build_fragment(xml, &json!({})).unwrap();

        assert_eq!(nodes.len(), 1);
        if let IRNode::Paragraph {
            style_override, ..
        } = &nodes[0]
        {
            assert_eq!(style_override.as_ref().unwrap().font_size, Some(16.0));
        } else {
            panic!("Expected IRNode::Paragraph");
        }
    }

    #[test]
    fn test_handle_list_and_list_item() {
        let xml = r#"<list><list-item>Item 1</list-item></list>"#;
        let nodes = build_fragment(xml, &json!({})).unwrap();

        assert_eq!(nodes.len(), 1);
        if let IRNode::List { children, .. } = &nodes[0] {
            assert_eq!(children.len(), 1);
            assert!(matches!(&children[0], IRNode::ListItem { .. }));
        } else {
            panic!("Expected IRNode::List");
        }
    }
}