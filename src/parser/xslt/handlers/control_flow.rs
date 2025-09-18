// src/parser/xslt/handlers/control_flow.rs
// src/parser/xslt/handlers/control_flow.rs
use super::super::builder::TreeBuilder;
use super::super::util::{get_attr_owned_required, OwnedAttributes};
use crate::error::PipelineError;
use crate::xpath;
use quick_xml::events::BytesStart;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;

impl<'a> TreeBuilder<'a> {
    pub(in crate::parser::xslt) fn handle_for_each(
        &mut self,
        _e: &BytesStart,
        reader: &mut Reader<&[u8]>,
        context: &'a Value,
        attributes: &OwnedAttributes,
    ) -> Result<(), PipelineError> {
        let path = get_attr_owned_required(attributes, b"select", b"xsl:for-each")?;
        log::debug!("Starting <xsl:for-each select=\"{}\">", path);
        let inner_xml =
            super::super::super::xslt::capture_inner_xml(reader, QName(b"xsl:for-each"))?;
        let selected_values = xpath::select(context, &path);
        let items: Vec<&'a Value> = if let Some(arr) =
            selected_values.first().and_then(|v| v.as_array())
        {
            arr.iter().collect()
        } else {
            selected_values
        };
        log::debug!("  <xsl:for-each> found {} items to iterate.", items.len());
        for (i, item_context) in items.iter().enumerate() {
            log::debug!("  <xsl:for-each> processing item {}", i);
            let mut template_reader = Reader::from_str(&inner_xml);
            template_reader.config_mut().trim_text(false);
            self.parse_nodes(&mut template_reader, item_context)?;
        }
        log::debug!("Finished <xsl:for-each select=\"{}\">", path);
        Ok(())
    }

    pub(in crate::parser::xslt) fn handle_if(
        &mut self,
        e: &BytesStart,
        reader: &mut Reader<&[u8]>,
        context: &Value,
        attributes: &OwnedAttributes,
    ) -> Result<(), PipelineError> {
        let test = get_attr_owned_required(attributes, b"test", b"xsl:if")?;
        let results = xpath::select(context, &test);
        let is_truthy = !results.is_empty()
            && results
            .iter()
            .all(|v| !v.is_null() && v.as_bool() != Some(false));
        log::debug!("<xsl:if test=\"{}\"> evaluated to {}", test, is_truthy);
        if !is_truthy {
            reader.read_to_end_into(e.name(), &mut vec![])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::idf::{IRNode, InlineNode};
    use crate::parser::xslt::handlers::test_helpers::build_fragment;
    use serde_json::json;

    fn get_text_content(node: &IRNode) -> String {
        if let IRNode::Paragraph { children, .. } = node {
            if let Some(InlineNode::Text(text)) = children.get(0) {
                return text.clone();
            }
        }
        String::new()
    }

    #[test]
    fn test_handle_if_true() {
        let xml = r#"<xsl:if test="show"><text>Visible</text></xsl:if>"#;
        let data = json!({ "show": true });
        let nodes = build_fragment(xml, &data).unwrap();

        assert_eq!(nodes.len(), 1);
        assert_eq!(get_text_content(&nodes[0]), "Visible");
    }

    #[test]
    fn test_handle_if_false() {
        let xml = r#"<xsl:if test="show"><text>Invisible</text></xsl:if>"#;
        let data = json!({ "show": false });
        let nodes = build_fragment(xml, &data).unwrap();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_handle_if_path_exists() {
        let xml = r#"<xsl:if test="user/name"><text>Exists</text></xsl:if>"#;
        let data = json!({ "user": { "name": "test" } });
        let nodes = build_fragment(xml, &data).unwrap();
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_handle_if_path_not_exists() {
        let xml = r#"<xsl:if test="user/email"><text>Exists</text></xsl:if>"#;
        let data = json!({ "user": { "name": "test" } });
        let nodes = build_fragment(xml, &data).unwrap();
        assert!(nodes.is_empty());
    }
}