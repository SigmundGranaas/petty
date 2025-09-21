// src/parser/xslt/handlers/special.rs

use super::super::builder::TreeBuilder;
use super::super::util::{
    get_attr_owned_optional, get_attr_owned_required, parse_fo_attributes_to_element_style,
    OwnedAttributes,
};
use crate::error::PipelineError;
use crate::idf::{IRNode, InlineNode};
use crate::xpath;
use serde_json::Value;

impl<'h> TreeBuilder<'h> {
    pub(in crate::parser::xslt) fn handle_value_of(
        &mut self,
        attributes: &OwnedAttributes,
        context: &Value,
    ) -> Result<(), PipelineError> {
        let path = get_attr_owned_required(attributes, b"select", b"xsl:value-of")?;
        let content = xpath::select_as_string(context, &path);
        log::trace!("  <xsl:value-of select=\"{}\"> -> \"{}\"", path, content);
        if !content.is_empty() {
            self.push_inline_to_parent(InlineNode::Text(content));
        }
        Ok(())
    }

    pub(in crate::parser::xslt) fn handle_image(
        &mut self,
        attributes: &OwnedAttributes,
        context: &Value,
    ) -> Result<(), PipelineError> {
        let src_template = get_attr_owned_required(attributes, b"src", b"image")?;
        let style_name = get_attr_owned_optional(attributes, b"style")?;
        let style_override = parse_fo_attributes_to_element_style(attributes)?;

        let src = self.render_text(&src_template, context)?;

        // An image is inline if it's inside a <text> (Paragraph) node.
        let in_paragraph = matches!(self.node_stack.last(), Some(IRNode::Paragraph { .. }));

        if in_paragraph {
            let node = InlineNode::Image {
                src,
                style_name,
                style_override,
                data: None,
            };
            self.push_inline_to_parent(node);
        } else {
            let node = IRNode::Image {
                src,
                style_name,
                style_override,
                data: None,
            };
            self.push_block_to_parent(node);
        }
        Ok(())
    }

    pub(in crate::parser::xslt) fn handle_line_break(&mut self) {
        self.push_inline_to_parent(InlineNode::LineBreak);
    }
}

#[cfg(test)]
mod tests {
    use crate::idf::{IRNode, InlineNode};
    use crate::parser::xslt::handlers::test_helpers::build_fragment;
    use serde_json::json;

    fn get_first_child_from_root(nodes: &[IRNode]) -> &IRNode {
        &nodes[0]
    }

    #[test]
    fn test_handle_value_of() {
        let xml = r#"<text>Hello <xsl:value-of select="name"/></text>"#;
        let nodes = build_fragment(xml, &json!({"name": "World"})).unwrap();
        let paragraph = get_first_child_from_root(&nodes);

        if let IRNode::Paragraph { children, .. } = paragraph {
            assert_eq!(children.len(), 2);
            assert!(matches!(&children[0], InlineNode::Text(t) if t == "Hello "));
            assert!(matches!(&children[1], InlineNode::Text(t) if t == "World"));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn test_handle_block_image() {
        let xml = r#"<image src="{{url}}"/>"#;
        let nodes = build_fragment(xml, &json!({"url": "image.png"})).unwrap();
        let image_node = get_first_child_from_root(&nodes);

        if let IRNode::Image { src, .. } = image_node {
            assert_eq!(src, "image.png");
        } else {
            panic!("Expected block-level image");
        }
    }

    #[test]
    fn test_handle_inline_image() {
        let xml = r#"<text>An image: <image src="icon.png"/></text>"#;
        let nodes = build_fragment(xml, &json!({})).unwrap();
        let paragraph = get_first_child_from_root(&nodes);

        if let IRNode::Paragraph { children, .. } = paragraph {
            assert_eq!(children.len(), 2);
            assert!(matches!(&children[1], InlineNode::Image{..}));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn test_handle_line_break() {
        let xml = r#"<text>Line 1<br/>Line 2</text>"#;
        let nodes = build_fragment(xml, &json!({})).unwrap();
        let paragraph = get_first_child_from_root(&nodes);

        if let IRNode::Paragraph { children, .. } = paragraph {
            assert_eq!(children.len(), 3);
            assert!(matches!(&children[1], InlineNode::LineBreak));
        } else {
            panic!("Expected paragraph");
        }
    }
}