// src/parser/xslt/handlers/inline.rs
// src/parser/xslt/handlers/inline.rs
use super::super::builder::TreeBuilder;
use super::super::util::{
    get_attr_owned_optional, get_attr_owned_required, parse_fo_attributes_to_element_style,
    OwnedAttributes,
};
use crate::error::PipelineError;
use crate::idf::InlineNode;
use serde_json::Value;

impl<'a> TreeBuilder<'a> {
    pub(in crate::parser::xslt) fn handle_link(
        &mut self,
        context: &Value,
        attributes: &OwnedAttributes,
    ) -> Result<(), PipelineError> {
        let href_template = get_attr_owned_required(attributes, b"href", b"link")?;
        let style_name = get_attr_owned_optional(attributes, b"style")?;
        let style_override = parse_fo_attributes_to_element_style(attributes)?;
        let href = self
            .template_engine
            .render_template(&href_template, context)
            .map_err(|err| PipelineError::TemplateParseError(err.to_string()))?;
        self.inline_stack.push(InlineNode::Hyperlink {
            href,
            style_name,
            style_override,
            children: vec![],
        });
        Ok(())
    }

    pub(in crate::parser::xslt) fn handle_styled_span(
        &mut self,
        tag_name: &[u8],
        attributes: &OwnedAttributes,
    ) -> Result<(), PipelineError> {
        let style_override = parse_fo_attributes_to_element_style(attributes)?;
        let style_name = match tag_name {
            b"strong" | b"b" => Some("bold".to_string()),
            b"em" | b"i" => Some("italic".to_string()),
            _ => None,
        };

        self.inline_stack.push(InlineNode::StyledSpan {
            style_name,
            style_override,
            children: vec![],
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::idf::{IRNode, InlineNode};
    use crate::parser::xslt::handlers::test_helpers::build_fragment;
    use serde_json::json;

    fn get_first_inline(nodes: &[IRNode]) -> &InlineNode {
        let paragraph = &nodes[0];
        if let IRNode::Paragraph { children, .. } = paragraph {
            return &children[0];
        }
        panic!("Expected paragraph node");
    }

    #[test]
    fn test_handle_link_with_templating() {
        let xml = r#"<text><link href="https://example.com/{{id}}">Link</link></text>"#;
        let nodes = build_fragment(xml, &json!({"id": 123})).unwrap();
        let inline = get_first_inline(&nodes);

        if let InlineNode::Hyperlink { href, .. } = inline {
            assert_eq!(href, "https://example.com/123");
        } else {
            panic!("Expected a Hyperlink node");
        }
    }

    #[test]
    fn test_handle_bold_span() {
        // The '#' in the color value requires an extra '#' in the raw string delimiter.
        let xml = r##"<text><strong color="#ff0000">Bold</strong></text>"##;
        let nodes = build_fragment(xml, &json!({})).unwrap();
        let inline = get_first_inline(&nodes);

        if let InlineNode::StyledSpan {
            style_name,
            style_override,
            ..
        } = inline
        {
            assert_eq!(style_name.as_deref(), Some("bold"));
            assert!(style_override.is_some());
            assert!(style_override.as_ref().unwrap().color.is_some());
        } else {
            panic!("Expected a StyledSpan node");
        }
    }

    #[test]
    fn test_handle_italic_span() {
        let xml = r#"<text><i>Italic</i></text>"#;
        let nodes = build_fragment(xml, &json!({})).unwrap();
        let inline = get_first_inline(&nodes);

        if let InlineNode::StyledSpan { style_name, .. } = inline {
            assert_eq!(style_name.as_deref(), Some("italic"));
        } else {
            panic!("Expected a StyledSpan node");
        }
    }
}