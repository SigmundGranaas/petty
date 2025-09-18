// src/parser/xslt/mod.rs

// src/parser/xslt/mod.rs
mod builder;
mod util;

use super::processor::TemplateProcessor;
use crate::error::PipelineError;
use crate::idf::{IRNode, LayoutUnit};
use crate::stylesheet::Stylesheet;
use crate::xpath;
use builder::TreeBuilder;
use handlebars::Handlebars;
use log;
use quick_xml::events::Event as XmlEvent;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;
use std::io::BufRead;

/// The main parser for XSLT-like templates. This struct acts as a factory
/// for creating an iterator that will produce one `IRNode` tree per `sequence`.
pub struct XsltTemplateParser {
    xslt_content: String,
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
}

impl XsltTemplateParser {
    pub fn new(
        xslt_content: String,
        stylesheet: Stylesheet,
        template_engine: Handlebars<'static>,
    ) -> Self {
        Self {
            xslt_content,
            stylesheet,
            template_engine,
        }
    }
}

impl TemplateProcessor for XsltTemplateParser {
    fn process<'a>(
        &'a mut self,
        data: &'a Value,
    ) -> Result<Box<dyn Iterator<Item = Result<LayoutUnit, PipelineError>> + 'a + Send>, PipelineError>
    {
        Ok(Box::new(XsltIterator::new(
            &self.xslt_content,
            data,
            &self.stylesheet,
            self.template_engine.clone(),
        )?))
    }
}

/// An iterator that lazily parses an XSLT template and produces a `LayoutUnit`
/// for each item found in the driving `<page-sequence>` tag.
pub struct XsltIterator<'a> {
    sequence_template: String,
    data_iterator: std::vec::IntoIter<&'a Value>,
    stylesheet: &'a Stylesheet,
    template_engine: Handlebars<'static>,
}

impl<'a> XsltIterator<'a> {
    fn new(
        xslt_content: &'a str,
        data: &'a Value,
        stylesheet: &'a Stylesheet,
        template_engine: Handlebars<'static>,
    ) -> Result<Self, PipelineError> {
        log::info!("Initializing XSLT iterator...");
        let mut reader = Reader::from_str(xslt_content);
        reader.config_mut().trim_text(false);
        let mut buf = Vec::new();

        // 1. Find the root template <xsl:template match="/">
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"xsl:template" => {
                    if e.attributes().flatten().any(|a| {
                        a.key.as_ref() == b"match" && a.value.as_ref() == b"/"
                    }) {
                        break; // Found it, now find the sequence inside
                    }
                }
                Ok(XmlEvent::Eof) => {
                    return Err(PipelineError::TemplateParseError(
                        "Could not find root <xsl:template match=\"/\">".to_string(),
                    ));
                }
                Err(e) => return Err(e.into()),
                _ => (),
            }
            buf.clear();
        }

        // 2. Find the <page-sequence> tag inside the root template
        let mut select_path: Option<String> = None;
        let mut sequence_template: Option<String> = None;
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"page-sequence" => {
                    select_path = Some(util::get_attr_required(&e, b"select")?);
                    sequence_template =
                        Some(capture_inner_xml(&mut reader, QName(b"page-sequence"))?);
                    break;
                }
                Ok(XmlEvent::End(e)) if e.name().as_ref() == b"xsl:template" => break, // End of root template
                Ok(XmlEvent::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => (),
            }
            buf.clear();
        }

        let path = select_path.ok_or_else(|| {
            PipelineError::TemplateParseError("Missing <page-sequence> in root template".into())
        })?;
        let template = sequence_template.ok_or_else(|| {
            PipelineError::TemplateParseError("Missing <page-sequence> in root template".into())
        })?;

        // 3. Select the data items to iterate over
        let selected_values = xpath::select(data, &path);
        let data_items: Vec<&'a Value> = if path == "." || path == "/" {
            vec![data]
        } else if let Some(first_val) = selected_values.get(0) {
            if let Some(arr) = first_val.as_array() {
                arr.iter().collect()
            } else {
                selected_values
            }
        } else {
            Vec::new()
        };

        log::info!(
            "Found <page-sequence select=\"{}\">, yielding {} sequences.",
            path,
            data_items.len()
        );

        Ok(Self {
            sequence_template: template,
            data_iterator: data_items.into_iter(),
            stylesheet,
            template_engine,
        })
    }
}

impl<'a> Iterator for XsltIterator<'a> {
    type Item = Result<LayoutUnit, PipelineError>;

    fn next(&mut self) -> Option<Self::Item> {
        let context = self.data_iterator.next()?;
        log::debug!("Building next sequence tree with context: {}", serde_json::to_string(context).unwrap_or_default());

        // Each sequence gets its own builder instance.
        let mut builder = TreeBuilder::new(&self.template_engine);

        // Parse the captured template fragment for the current data context.
        let result = builder.build_tree_from_xml_str(&self.sequence_template, context);

        let tree = match result {
            Ok(root_node) => root_node,
            Err(e) => return Some(Err(e)),
        };

        log::debug!("Successfully built sequence tree.");
        Some(Ok(LayoutUnit {
            tree: IRNode::Root(tree),
            context: context.clone(),
        }))
    }
}

/// A utility to capture the inner raw XML of a node.
/// Assumes the reader is positioned after the Start tag.
pub(crate) fn capture_inner_xml<B: BufRead>(
    reader: &mut Reader<B>,
    tag_name: QName,
) -> Result<String, PipelineError> {
    let mut buf = Vec::new();
    let mut writer_buf = Vec::new();
    let mut writer = quick_xml::Writer::new(&mut writer_buf);
    let mut depth = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => {
                // The original tag was already consumed, so the first tag with this name
                // increases the depth.
                if e.name() == tag_name {
                    depth += 1;
                }
                writer.write_event(XmlEvent::Start(e))?;
            }
            Ok(XmlEvent::End(e)) => {
                if e.name() == tag_name {
                    if depth == 0 {
                        // This is the closing tag for the original node.
                        break;
                    }
                    // This is a closing tag for a nested node of the same name.
                    depth -= 1;
                }
                writer.write_event(XmlEvent::End(e))?;
            }
            Ok(XmlEvent::Eof) => {
                return Err(PipelineError::TemplateParseError(
                    "Unclosed tag while capturing inner XML".into(),
                ))
            }
            Ok(event) => {
                writer.write_event(event)?;
            }
            Err(e) => return Err(e.into()),
        }
        buf.clear();
    }
    drop(writer);
    Ok(String::from_utf8(writer_buf)?)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::InlineNode;
    use handlebars::Handlebars;
    use serde_json::json;

    fn generate_large_test_data(count: usize) -> Value {
        let records: Vec<Value> = (0..count).map(|i| json!({ "id": i })).collect();
        json!({ "records": records })
    }

    #[test]
    fn test_xslt_parser_is_lazy_and_streams_data() {
        let num_records = 10_000;
        let data = generate_large_test_data(num_records);

        let xslt_content = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:fo="http://www.w3.org/1999/XSL/Format">
                <fo:simple-page-master page-width="8.5in" page-height="11in" />
                <xsl:template match="/">
                    <page-sequence select="records">
                        <container>
                            <text>Record ID: <xsl:value-of select="id"/></text>
                        </container>
                    </page-sequence>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = Stylesheet::from_xslt(xslt_content).unwrap();
        let handlebars = Handlebars::new();
        let mut parser = XsltTemplateParser::new(xslt_content.to_string(), stylesheet, handlebars);

        // `process` should return quickly, creating the iterator.
        let mut iterator_result = parser.process(&data).unwrap();

        // Consume the iterator.
        let mut count = 0;
        let mut first_context: Option<Value> = None;
        let mut last_context: Option<Value> = None;

        while let Some(item_result) = iterator_result.next() {
            let layout_unit = item_result.expect("LayoutUnit should be generated successfully");
            if count == 0 {
                first_context = Some(layout_unit.context.clone());
            }
            last_context = Some(layout_unit.context.clone());
            count += 1;
        }

        // Assert that the iterator produced one LayoutUnit for each record.
        assert_eq!(count, num_records, "The iterator should produce one LayoutUnit per record");

        // Assert the context of the first and last items.
        assert_eq!(first_context, Some(json!({"id": 0})));
        assert_eq!(last_context, Some(json!({"id": num_records - 1})));
    }

    #[test]
    fn test_xslt_perf_template_iteration() {
        let data = json!({
            "records": [
                { "user": { "account": "ACC-1" } },
                { "user": { "account": "ACC-2" } }
            ]
        });

        // A simplified version of the perf_test_template.xsl
        let xslt_content = r#"
            <xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:fo="http://www.w3.org/1999/XSL/Format">
                <fo:simple-page-master page-width="8.5in" page-height="11in" />
                <xsl:template match="/">
                    <page-sequence select="records">
                        <text>Account: <xsl:value-of select="user/account"/></text>
                    </page-sequence>
                </xsl:template>
            </xsl:stylesheet>
        "#;

        let stylesheet = Stylesheet::from_xslt(xslt_content).unwrap();
        let handlebars = Handlebars::new();
        let mut parser = XsltTemplateParser::new(xslt_content.to_string(), stylesheet, handlebars);

        let mut iterator = parser.process(&data).unwrap();

        // Check first layout unit
        let unit1 = iterator.next().unwrap().unwrap();
        assert_eq!(unit1.context, json!({ "user": { "account": "ACC-1" } }));
        if let IRNode::Root(children) = unit1.tree {
            assert_eq!(children.len(), 1);
            if let IRNode::Paragraph { children: inlines, .. } = &children[0] {
                // Expecting "Account: " and "ACC-1" as two separate inline text nodes
                assert_eq!(inlines.len(), 2, "Expected two inline text nodes for static text and value-of");
                if let (Some(InlineNode::Text(t1)), Some(InlineNode::Text(t2))) = (inlines.get(0), inlines.get(1)) {
                    assert_eq!(t1, "Account: ");
                    assert_eq!(t2, "ACC-1");
                } else {
                    panic!("Expected two text inlines, got: {:?}", inlines);
                }
            } else {
                panic!("Expected a Paragraph node");
            }
        } else {
            panic!("Expected a Root node");
        }

        // Check second layout unit
        let unit2 = iterator.next().unwrap().unwrap();
        assert_eq!(unit2.context, json!({ "user": { "account": "ACC-2" } }));

        // Check for end of iteration
        assert!(iterator.next().is_none(), "Iterator should be exhausted");
    }
}