// src/events.rs

use crate::errors::PipelineError;
use crate::stylesheet::{Stylesheet, TableColumn, TemplateElement};
use handlebars::Handlebars;
use serde_json::Value;

/// Represents a command to the layout engine, forming the event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateEvent<'a> {
    StartDocument,
    EndDocument,
    BeginPageSequenceItem { context: &'a Value },
    EndPageSequenceItem,
    StartContainer { style: Option<String> },
    EndContainer,
    AddText { content: String, style: Option<String> },
    AddRectangle { style: Option<String> },
    StartTable {
        style: Option<String>,
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
        content: String,
        style_override: Option<String>,
    },
    EndRow,
    EndTable,
    ForcePageBreak,
}

/// Parses stylesheet templates and data into a stream of `TemplateEvent`s.
pub struct EventParser<'a> {
    stylesheet: &'a Stylesheet,
    template_engine: &'a Handlebars<'static>,
}

impl<'a> EventParser<'a> {
    pub fn new(stylesheet: &'a Stylesheet, template_engine: &'a Handlebars<'static>) -> Self {
        EventParser {
            stylesheet,
            template_engine,
        }
    }

    pub fn parse(&self, data: &'a Value) -> Result<Vec<TemplateEvent<'a>>, PipelineError> {
        let mut events = Vec::with_capacity(1024);
        events.push(TemplateEvent::StartDocument);

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
                events.push(TemplateEvent::BeginPageSequenceItem {
                    context: item_data,
                });
                self.parse_children(&template.children, item_data, &mut events)?;
                events.push(TemplateEvent::EndPageSequenceItem);
            }
        }

        events.push(TemplateEvent::EndDocument);
        Ok(events)
    }

    fn parse_children(
        &self,
        children: &'a [TemplateElement],
        context: &'a Value,
        events: &mut Vec<TemplateEvent<'a>>,
    ) -> Result<(), PipelineError> {
        for element in children {
            match element {
                TemplateElement::Text {
                    content,
                    style,
                    template_name,
                } => {
                    let rendered_content = if let Some(name) = template_name {
                        self.template_engine
                            .render(name, context)
                            .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?
                    } else {
                        content.clone()
                    };
                    events.push(TemplateEvent::AddText {
                        content: rendered_content,
                        style: style.clone(),
                    });
                }
                TemplateElement::Rectangle { style } => {
                    events.push(TemplateEvent::AddRectangle {
                        style: style.clone(),
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
                        events.push(TemplateEvent::StartContainer {
                            style: style.clone(),
                        });
                        self.parse_children(children, item_context, events)?;
                        events.push(TemplateEvent::EndContainer);
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
                        events,
                    )?;
                }
                TemplateElement::PageBreak => {
                    events.push(TemplateEvent::ForcePageBreak);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_table(
        &self,
        data_source: &'a str,
        columns: &'a [TableColumn],
        style: &'a Option<String>,
        row_style_prefix_field: &'a Option<String>,
        context: &'a Value,
        events: &mut Vec<TemplateEvent<'a>>,
    ) -> Result<(), PipelineError> {
        events.push(TemplateEvent::StartTable {
            style: style.clone(),
            columns,
        });

        events.push(TemplateEvent::StartHeader);
        events.push(TemplateEvent::StartRow {
            context: &Value::Null,
            row_style_prefix: None,
        });
        for (i, col) in columns.iter().enumerate() {
            events.push(TemplateEvent::AddCell {
                column_index: i,
                content: col.header.clone(),
                style_override: col.header_style.clone(),
            });
        }
        events.push(TemplateEvent::EndRow);
        events.push(TemplateEvent::EndHeader);

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

            events.push(TemplateEvent::StartRow {
                context: row_item,
                row_style_prefix: prefix,
            });

            for (i, col) in columns.iter().enumerate() {
                let cell_value = row_item.pointer(&col.data_field).unwrap_or(&Value::Null);

                let cell_text = if let Some(name) = &col.template_name {
                    self.template_engine
                        .render(name, row_item)
                        .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?
                } else {
                    match cell_value {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => "".to_string(),
                    }
                };

                let final_style = if let (Some(prefix), Some(base_style)) =
                    (row_item.get("type").and_then(|v| v.as_str()), &col.style)
                {
                    Some(format!("{}-{}", prefix, base_style))
                } else {
                    col.style.clone()
                };

                events.push(TemplateEvent::AddCell {
                    column_index: i,
                    content: cell_text,
                    style_override: final_style,
                });
            }
            events.push(TemplateEvent::EndRow);
        }

        events.push(TemplateEvent::EndTable);
        Ok(())
    }
}