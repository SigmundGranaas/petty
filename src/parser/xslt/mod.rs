pub mod builder;
mod handlers;
mod util;

use super::processor::TemplateProcessor;
use crate::error::PipelineError;
use crate::idf::{IRNode, LayoutUnit};
use crate::stylesheet::{PageSequence, Stylesheet};
use crate::xpath;
pub use builder::PreparsedTemplate;
use builder::TreeBuilder;
use handlebars::Handlebars;
use log;
use quick_xml::events::Event as XmlEvent;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub struct XsltTemplateParser {
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
    preparsed_template: PreparsedTemplate,
}

impl XsltTemplateParser {
    pub fn new(
        xslt_content: String,
        stylesheet: Stylesheet,
        template_engine: Handlebars<'static>,
    ) -> Result<Self, PipelineError> {
        let sequence_template_str = extract_sequence_template(&xslt_content)?;
        let builder = TreeBuilder::new(&template_engine);
        let preparsed_template = builder.preparse_from_str(&sequence_template_str)?;

        Ok(Self {
            stylesheet,
            template_engine,
            preparsed_template,
        })
    }
}

impl TemplateProcessor for XsltTemplateParser {
    fn process<'a>(
        &'a mut self,
        data: &'a Value,
    ) -> Result<Box<dyn Iterator<Item = Result<LayoutUnit, PipelineError>> + 'a + Send>, PipelineError>
    {
        Ok(Box::new(XsltIterator::new(
            &self.preparsed_template,
            data,
            &self.stylesheet,
            &self.template_engine,
        )?))
    }
}

pub struct XsltIterator<'a> {
    preparsed_template: &'a PreparsedTemplate,
    data_iterator: std::vec::IntoIter<&'a Value>,
    stylesheet: &'a Stylesheet,
    builder: TreeBuilder<'a>,
}

impl<'a> XsltIterator<'a> {
    fn new(
        preparsed_template: &'a PreparsedTemplate,
        data: &'a Value,
        stylesheet: &'a Stylesheet,
        template_engine: &'a Handlebars<'static>,
    ) -> Result<Self, PipelineError> {
        log::info!("Initializing XSLT iterator...");

        let path = get_sequence_path(&stylesheet.page_sequences)?;

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

        let builder = TreeBuilder::new(template_engine);

        Ok(Self {
            preparsed_template,
            data_iterator: data_items.into_iter(),
            stylesheet,
            builder,
        })
    }
}

pub fn get_sequence_path(
    page_sequences: &HashMap<String, PageSequence>,
) -> Result<String, PipelineError> {
    let seq = page_sequences.values().next().ok_or_else(|| {
        PipelineError::StylesheetError("No <page-sequence> defined in template.".to_string())
    })?;
    Ok(seq.data_source.clone())
}

pub fn extract_sequence_template(xslt_content: &str) -> Result<String, PipelineError> {
    let mut reader = Reader::from_str(xslt_content);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"xsl:template" => {
                if e.attributes().flatten().any(|a| {
                    a.key.as_ref() == b"match" && a.value.as_ref() == b"/"
                }) {
                    break;
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

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"page-sequence" => {
                return Ok(util::capture_inner_xml(
                    &mut reader,
                    QName(b"page-sequence"),
                )?);
            }
            Ok(XmlEvent::End(e)) if e.name().as_ref() == b"xsl:template" => break,
            Ok(XmlEvent::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => (),
        }
        buf.clear();
    }
    Err(PipelineError::TemplateParseError(
        "Missing <page-sequence> in root template".into(),
    ))
}

impl<'a> Iterator for XsltIterator<'a> {
    type Item = Result<LayoutUnit, PipelineError>;

    fn next(&mut self) -> Option<Self::Item> {
        let context = self.data_iterator.next()?;
        log::debug!(
            "Building next sequence tree with context: {}",
            serde_json::to_string(context).unwrap_or_default()
        );

        let result = self
            .builder
            .build_tree_from_preparsed(self.preparsed_template, context);

        let tree = match result {
            Ok(root_node) => root_node,
            Err(e) => return Some(Err(e)),
        };

        log::debug!("Successfully built sequence tree.");
        Some(Ok(LayoutUnit {
            tree: IRNode::Root(tree),
            context: Arc::new(context.clone()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idf::InlineNode;
    use handlebars::Handlebars;
    use serde_json::json;
    use std::sync::Arc;

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
        let mut parser =
            XsltTemplateParser::new(xslt_content.to_string(), stylesheet, handlebars).unwrap();

        let mut iterator_result = parser.process(&data).unwrap();

        let mut count = 0;
        let mut first_context: Option<Arc<Value>> = None;
        let mut last_context: Option<Arc<Value>> = None;

        while let Some(item_result) = iterator_result.next() {
            let layout_unit = item_result.expect("LayoutUnit should be generated successfully");
            if count == 0 {
                first_context = Some(layout_unit.context.clone());
            }
            last_context = Some(layout_unit.context.clone());
            count += 1;
        }

        assert_eq!(
            count,
            num_records,
            "The iterator should produce one LayoutUnit per record"
        );
        assert_eq!(*first_context.unwrap(), json!({"id": 0}));
        assert_eq!(*last_context.unwrap(), json!({"id": num_records - 1}));
    }

    #[test]
    fn test_xslt_perf_template_iteration() {
        let data = json!({
            "records": [
                { "user": { "account": "ACC-1" } },
                { "user": { "account": "ACC-2" } }
            ]
        });

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
        let mut parser =
            XsltTemplateParser::new(xslt_content.to_string(), stylesheet, handlebars).unwrap();

        let mut iterator = parser.process(&data).unwrap();

        let unit1 = iterator.next().unwrap().unwrap();
        assert_eq!(*unit1.context, json!({ "user": { "account": "ACC-1" } }));
        if let IRNode::Root(children) = unit1.tree {
            assert_eq!(children.len(), 1);
            if let IRNode::Paragraph {
                children: inlines, ..
            } = &children[0]
            {
                assert_eq!(
                    inlines.len(),
                    2,
                    "Expected two inline text nodes for static text and value-of"
                );
                if let (Some(InlineNode::Text(t1)), Some(InlineNode::Text(t2))) =
                    (inlines.get(0), inlines.get(1))
                {
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

        let unit2 = iterator.next().unwrap().unwrap();
        assert_eq!(*unit2.context, json!({ "user": { "account": "ACC-2" } }));
        assert!(iterator.next().is_none(), "Iterator should be exhausted");
    }
}