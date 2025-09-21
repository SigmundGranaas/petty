// src/parser/json_processor.rs

use super::processor::TemplateProcessor;
use crate::error::PipelineError;
use crate::idf::{
    IRNode, InlineNode, LayoutUnit, TableBody, TableCell, TableColumnDefinition, TableHeader,
    TableRow,
};
use crate::stylesheet::{Stylesheet, TableColumn, Template, TemplateElement};
use handlebars::Handlebars;
use serde_json::Value;
use std::borrow::Cow;
use std::vec;

/// Processes templates defined in the main JSON stylesheet into `IRNode` trees.
pub struct JsonTemplateParser {
    stylesheet: Stylesheet,
    template_engine: Handlebars<'static>,
}

impl JsonTemplateParser {
    pub fn new(stylesheet: Stylesheet, template_engine: Handlebars<'static>) -> Self {
        JsonTemplateParser {
            stylesheet,
            template_engine,
        }
    }

    /// Recursively builds a vector of `IRNode`s from a list of template elements
    /// within a given data context.
    fn build_children(
        &self,
        children: &[TemplateElement],
        context: &Value,
    ) -> Result<Vec<IRNode>, PipelineError> {
        let mut nodes = Vec::new();
        for element in children {
            match element {
                TemplateElement::Text { content, style } => {
                    let rendered_content = if content.contains("{{") {
                        self.template_engine
                            .render_template(content, context)
                            .map_err(|e| {
                                PipelineError::TemplateParseError(format!(
                                    "Failed to render template '{}': {}",
                                    content, e
                                ))
                            })?
                    } else {
                        content.clone()
                    };
                    nodes.push(IRNode::Paragraph {
                        style_name: style.clone(),
                        style_override: None,
                        children: vec![InlineNode::Text(rendered_content)],
                    });
                }
                TemplateElement::Rectangle { style } => {
                    // Rectangles are typically style-only, represented as a styled Block.
                    nodes.push(IRNode::Block {
                        style_name: style.clone(),
                        style_override: None,
                        children: vec![],
                    });
                }
                TemplateElement::Container {
                    children,
                    style,
                    data_source,
                } => {
                    let data_items: Vec<&Value> = if let Some(ds) = data_source {
                        context
                            .pointer(ds)
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().collect())
                            .unwrap_or_else(|| vec![context])
                    } else {
                        vec![context]
                    };

                    for item_context in data_items {
                        nodes.push(IRNode::Block {
                            style_name: style.clone(),
                            style_override: None,
                            children: self.build_children(children, item_context)?,
                        });
                    }
                }
                TemplateElement::Table {
                    data_source,
                    columns,
                    style,
                    row_style_prefix_field,
                } => {
                    nodes.push(self.build_table(
                        data_source,
                        columns,
                        style,
                        row_style_prefix_field,
                        context,
                    )?);
                }
                TemplateElement::Image { src, style } => {
                    let rendered_src = if src.contains("{{") {
                        self.template_engine.render_template(src, context).map_err(
                            |e| {
                                PipelineError::TemplateParseError(format!(
                                    "Failed to render image src '{}': {}",
                                    src, e
                                ))
                            },
                        )?
                    } else {
                        src.clone()
                    };
                    nodes.push(IRNode::Image {
                        src: rendered_src,
                        style_name: style.clone(),
                        style_override: None,
                        data: None, // Will be populated by ResourceManager
                    });
                }
                TemplateElement::PageBreak => {
                    // Page breaks are handled by the layout engine, not represented in the IR tree.
                    // This is a philosophical shift. If a hard break is needed, it should be
                    // a property on a node, e.g., `break-before: page`. For now, we ignore it.
                    log::warn!("PageBreak elements are currently ignored during IR construction.");
                }
            }
        }
        Ok(nodes)
    }

    /// Handles the building of a `Table` IRNode.
    fn build_table(
        &self,
        data_source: &str,
        columns: &[TableColumn],
        style: &Option<String>,
        row_style_prefix_field: &Option<String>,
        context: &Value,
    ) -> Result<IRNode, PipelineError> {
        // Build header
        let header_rows: Vec<TableRow> = vec![TableRow {
            cells: columns
                .iter()
                .map(|col| TableCell {
                    style_name: col.header_style.clone(),
                    style_override: None,
                    children: vec![IRNode::Paragraph {
                        style_name: None, // Inherits from cell style
                        style_override: None,
                        children: vec![InlineNode::Text(col.header.clone())],
                    }],
                })
                .collect(),
        }];
        let header = if columns.iter().any(|c| !c.header.is_empty()) {
            Some(Box::new(TableHeader { rows: header_rows }))
        } else {
            None
        };


        // Build body
        let rows_data: Vec<&Value> = if data_source == "/" || data_source == "." {
            // Special case: if data_source is the root, treat the context as a single-row data source.
            vec![context]
        } else {
            context
                .pointer(data_source)
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    PipelineError::TemplateParseError(format!(
                        "Table data source '{}' not found or is not an array in context",
                        data_source
                    ))
                })?
                .iter()
                .collect()
        };


        let body_rows: Vec<TableRow> = rows_data
            .iter()
            .map(|&row_item| {
                let prefix_str = row_style_prefix_field
                    .as_ref()
                    .and_then(|field| row_item.pointer(field).and_then(|v| v.as_str()));

                let cells = columns
                    .iter()
                    .map(|col| {
                        let cell_value = row_item.pointer(&col.data_field).unwrap_or(&Value::Null);

                        let cell_text: Cow<'_, str> =
                            if let Some(template_string) = &col.content_template {
                                Cow::Owned(
                                    self.template_engine
                                        .render_template(template_string, row_item)
                                        .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?,
                                )
                            } else {
                                match cell_value {
                                    Value::String(s) => Cow::Borrowed(s),
                                    Value::Number(n) => Cow::Owned(n.to_string()),
                                    Value::Bool(b) => Cow::Owned(b.to_string()),
                                    _ => Cow::Borrowed(""),
                                }
                            };

                        let final_style = if let (Some(prefix), Some(base_style)) =
                            (prefix_str, &col.style)
                        {
                            Some(format!("{}-{}", prefix, base_style))
                        } else {
                            col.style.clone()
                        };

                        Ok(TableCell {
                            style_name: final_style,
                            style_override: None,
                            children: vec![IRNode::Paragraph {
                                style_name: None,
                                style_override: None,
                                children: vec![InlineNode::Text(cell_text.into_owned())],
                            }],
                        })
                    })
                    .collect::<Result<Vec<TableCell>, PipelineError>>()?;
                Ok(TableRow { cells })
            })
            .collect::<Result<Vec<TableRow>, PipelineError>>()?;

        let body = Box::new(TableBody { rows: body_rows });

        let col_defs = columns
            .iter()
            .map(|c| TableColumnDefinition {
                width: c.width.clone(),
                style: c.style.clone(),
                header_style: c.header_style.clone(),
            })
            .collect();

        Ok(IRNode::Table {
            style_name: style.clone(),
            style_override: None,
            columns: col_defs,
            calculated_widths: vec![], // To be filled by layout engine
            header,
            body,
        })
    }
}

/// A lazy iterator that builds one `LayoutUnit` at a time.
struct JsonIterator<'a> {
    parser: &'a JsonTemplateParser,
    items_to_process: vec::IntoIter<(&'a Template, &'a Value)>,
}

impl<'a> Iterator for JsonIterator<'a> {
    type Item = Result<LayoutUnit, PipelineError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (template, item_data) = self.items_to_process.next()?;

        let result = self
            .parser
            .build_children(&template.children, item_data)
            .map(|children| {
                let tree = IRNode::Root(children);
                LayoutUnit {
                    tree,
                    context: item_data.clone().into(),
                }
            });

        Some(result)
    }
}

impl TemplateProcessor for JsonTemplateParser {
    fn process<'a>(
        &'a mut self,
        data: &'a Value,
    ) -> Result<Box<dyn Iterator<Item = Result<LayoutUnit, PipelineError>> + 'a + Send>, PipelineError>
    {
        if self.stylesheet.page_sequences.is_empty() {
            return Err(PipelineError::StylesheetError(
                "No page_sequences defined in stylesheet.".to_string(),
            ));
        }

        let mut items_to_process = Vec::new();

        for (_seq_name, sequence) in &self.stylesheet.page_sequences {
            let template: &Template =
                self.stylesheet.templates.get(&sequence.template).ok_or_else(
                    || {
                        PipelineError::TemplateParseError(format!(
                            "Template '{}' not found",
                            sequence.template
                        ))
                    },
                )?;

            let data_items: Vec<&Value> = if sequence.data_source == "/" {
                vec![data]
            } else {
                data.pointer(&sequence.data_source)
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().collect())
                    .ok_or_else(|| {
                        PipelineError::TemplateParseError(format!(
                            "Data source '{}' not found or not an array",
                            sequence.data_source
                        ))
                    })?
            };

            for item_data in data_items {
                items_to_process.push((template, item_data));
            }
        }

        Ok(Box::new(JsonIterator {
            parser: self,
            items_to_process: items_to_process.into_iter(),
        }))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::stylesheet::{PageSequence, Template};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_test_stylesheet(
        template_name: &str,
        data_source: &str,
        template: Template,
    ) -> Stylesheet {
        let mut templates = HashMap::new();
        templates.insert(template_name.to_string(), template);

        let mut page_sequences = HashMap::new();
        page_sequences.insert(
            "main".to_string(),
            PageSequence {
                template: template_name.to_string(),
                data_source: data_source.to_string(),
            },
        );

        Stylesheet {
            templates,
            page_sequences,
            ..Default::default()
        }
    }

    fn generate_large_test_data(count: usize) -> Value {
        let records: Vec<Value> = (0..count).map(|i| json!({ "id": i })).collect();
        json!({ "records": records })
    }

    #[test]
    fn test_json_parser_is_lazy_and_streams_data() {
        let num_records = 10_000;
        let data = generate_large_test_data(num_records);

        let template = Template {
            name: "recordPage".to_string(),
            children: vec![TemplateElement::Text {
                content: "ID: {{id}}".to_string(),
                style: None,
            }],
        };

        let stylesheet = create_test_stylesheet("recordPage", "/records", template);
        let handlebars = Handlebars::new();
        let mut parser = JsonTemplateParser::new(stylesheet, handlebars);

        // The `process` call should be very fast as it only sets up the iterator.
        let mut iterator_result = parser.process(&data).unwrap();

        // Consume the iterator and verify its output.
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

        // Assert that the iterator produced one LayoutUnit for each record.
        assert_eq!(
            count, num_records,
            "The iterator should produce one LayoutUnit per record"
        );

        // Assert the context of the first and last items to ensure correct data processing.
        assert_eq!(*first_context.unwrap(), json!({"id": 0}));
        assert_eq!(*last_context.unwrap(), json!({"id": num_records - 1}));
    }
}