// src/parser/mod.rs
use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::render::DocumentRenderer;
use crate::stylesheet::{Stylesheet, TableColumn, TemplateElement};
use handlebars::Handlebars;
use serde_json::Value;
use std::borrow::Cow;

/// Represents a high-level command for the layout engine, forming an event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Event<'a> {
    StartDocument,
    EndDocument,
    BeginPageSequenceItem {
        context: &'a Value,
    },
    EndPageSequenceItem,
    StartContainer {
        style: Option<&'a str>,
    },
    EndContainer,
    AddText {
        content: Cow<'a, str>,
        style: Option<&'a str>,
    },
    AddRectangle {
        style: Option<&'a str>,
    },
    StartTable {
        style: Option<&'a str>,
        columns: &'a [TableColumn],
    },
    StartHeader,
    EndHeader,
    StartRow {
        context: &'a Value,
        row_style_prefix: Option<String>,
    },
    AddCell {
        column_index: usize,
        content: Cow<'a, str>,
        style_override: Option<String>,
    },
    EndRow,
    EndTable,
    ForcePageBreak,
}

/// Parses stylesheet templates and data, feeding `Event`s directly to a processor.
pub struct Parser<'a> {
    stylesheet: &'a Stylesheet,
    template_engine: &'a Handlebars<'static>,
}

impl<'a> Parser<'a> {
    pub fn new(stylesheet: &'a Stylesheet, template_engine: &'a Handlebars<'static>) -> Self {
        Parser {
            stylesheet,
            template_engine,
        }
    }

    pub fn parse<R: DocumentRenderer<'a>>(
        &self,
        data: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError> {
        processor.process_event(Event::StartDocument)?;

        if self.stylesheet.page_sequences.is_empty() {
            return Err(PipelineError::StylesheetError(
                "No page_sequences defined in stylesheet.".to_string(),
            ));
        }

        for (_seq_name, sequence) in &self.stylesheet.page_sequences {
            let template = self
                .stylesheet
                .templates
                .get(&sequence.template)
                .ok_or_else(|| {
                    PipelineError::TemplateParseError(format!(
                        "Template '{}' not found",
                        sequence.template
                    ))
                })?;

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
                processor.process_event(Event::BeginPageSequenceItem { context: item_data })?;
                self.parse_children(&template.children, item_data, processor)?;
                processor.process_event(Event::EndPageSequenceItem)?;
            }
        }

        processor.process_event(Event::EndDocument)?;
        Ok(())
    }

    fn parse_children<R: DocumentRenderer<'a>>(
        &self,
        children: &'a [TemplateElement],
        context: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError> {
        for element in children {
            match element {
                TemplateElement::Text {
                    content,
                    style,
                    template_name,
                } => {
                    let rendered_content: Cow<'a, str> = if let Some(name) = template_name {
                        Cow::Owned(self.template_engine.render(name, context).map_err(|e| {
                            PipelineError::TemplateParseError(e.to_string())
                        })?)
                    } else {
                        Cow::Borrowed(content)
                    };
                    processor.process_event(Event::AddText {
                        content: rendered_content,
                        style: style.as_deref(),
                    })?;
                }
                TemplateElement::Rectangle { style } => {
                    processor.process_event(Event::AddRectangle {
                        style: style.as_deref(),
                    })?;
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
                        processor.process_event(Event::StartContainer {
                            style: style.as_deref(),
                        })?;
                        self.parse_children(children, item_context, processor)?;
                        processor.process_event(Event::EndContainer)?;
                    }
                }
                TemplateElement::Table {
                    data_source,
                    columns,
                    style,
                    row_style_prefix_field,
                } => {
                    self.parse_table(
                        data_source,
                        columns,
                        style,
                        row_style_prefix_field,
                        context,
                        processor,
                    )?;
                }
                TemplateElement::PageBreak => {
                    processor.process_event(Event::ForcePageBreak)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_table<R: DocumentRenderer<'a>>(
        &self,
        data_source: &'a str,
        columns: &'a [TableColumn],
        style: &'a Option<String>,
        row_style_prefix_field: &'a Option<String>,
        context: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError> {
        processor.process_event(Event::StartTable {
            style: style.as_deref(),
            columns,
        })?;

        processor.process_event(Event::StartHeader)?;
        processor.process_event(Event::StartRow {
            context: &Value::Null,
            row_style_prefix: None,
        })?;
        for (i, col) in columns.iter().enumerate() {
            processor.process_event(Event::AddCell {
                column_index: i,
                content: Cow::Borrowed(&col.header),
                style_override: col.header_style.clone(),
            })?;
        }
        processor.process_event(Event::EndRow)?;
        processor.process_event(Event::EndHeader)?;

        let rows_data = context
            .pointer(data_source)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                PipelineError::TemplateParseError(format!(
                    "Table data source '{}' not found or is not an array in context",
                    data_source
                ))
            })?;

        for row_item in rows_data {
            let prefix = row_style_prefix_field.as_ref().and_then(|field| {
                row_item
                    .pointer(field)
                    .and_then(|v| v.as_str().map(String::from))
            });

            processor.process_event(Event::StartRow {
                context: row_item,
                row_style_prefix: prefix,
            })?;

            for (i, col) in columns.iter().enumerate() {
                let cell_value = row_item.pointer(&col.data_field).unwrap_or(&Value::Null);

                let cell_text: Cow<'a, str> = if let Some(name) = &col.template_name {
                    Cow::Owned(self.template_engine.render(name, row_item).map_err(|e| {
                        PipelineError::TemplateParseError(e.to_string())
                    })?)
                } else {
                    match cell_value {
                        Value::String(s) => Cow::Borrowed(s),
                        Value::Number(n) => Cow::Owned(n.to_string()),
                        Value::Bool(b) => Cow::Owned(b.to_string()),
                        _ => Cow::Borrowed(""),
                    }
                };

                let final_style = if let (Some(prefix), Some(base_style)) =
                    (row_item.get("type").and_then(|v| v.as_str()), &col.style)
                {
                    Some(format!("{}-{}", prefix, base_style))
                } else {
                    col.style.clone()
                };

                processor.process_event(Event::AddCell {
                    column_index: i,
                    content: cell_text,
                    style_override: final_style,
                })?;
            }
            processor.process_event(Event::EndRow)?;
        }

        processor.process_event(Event::EndTable)?;
        Ok(())
    }
}