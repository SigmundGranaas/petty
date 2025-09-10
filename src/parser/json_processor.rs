// src/parser/json_processor.rs
use super::processor::{LayoutProcessorProxy, TemplateProcessor};
use crate::error::PipelineError;
use crate::idf::IDFEvent;
use crate::stylesheet::{Stylesheet, TableColumn, Template, TemplateElement};
use async_trait::async_trait;
use handlebars::Handlebars;
use serde_json::Value;
use std::borrow::Cow;

/// Processes templates defined in the main JSON stylesheet.
pub struct JsonTemplateParser<'a> {
    stylesheet: &'a Stylesheet,
    template_engine: &'a Handlebars<'static>,
}

impl<'a> JsonTemplateParser<'a> {
    pub fn new(stylesheet: &'a Stylesheet, template_engine: &'a Handlebars<'static>) -> Self {
        JsonTemplateParser {
            stylesheet,
            template_engine,
        }
    }

    /// Recursively parses a list of template elements within a given data context,
    /// emitting events to the layout processor.
    async fn parse_children(
        &mut self,
        children: &'a [TemplateElement],
        context: &'a Value,
        proxy: &mut LayoutProcessorProxy<'a>,
    ) -> Result<(), PipelineError> {
        for element in children {
            match element {
                TemplateElement::Text { content, style } => {
                    let rendered_content: Cow<'a, str> = if content.contains("{{") {
                        Cow::Owned(self.template_engine.render_template(content, context)
                            .map_err(|e| PipelineError::TemplateParseError(format!("Failed to render template '{}': {}", content, e)))?)
                    } else {
                        Cow::Borrowed(content)
                    };
                    proxy.process_event(IDFEvent::AddText {
                        content: rendered_content,
                        style: style.as_deref().map(Cow::Borrowed),
                    }).await?;
                }
                TemplateElement::Rectangle { style } => {
                    proxy.process_event(IDFEvent::AddRectangle {
                        style: style.as_deref().map(Cow::Borrowed),
                    }).await?;
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
                        proxy.process_event(IDFEvent::StartBlock {
                            style: style.as_deref().map(Cow::Borrowed),
                        }).await?;
                        Box::pin(self.parse_children(children, item_context, proxy)).await?;
                        proxy.process_event(IDFEvent::EndBlock).await?;
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
                        proxy,
                    )
                        .await?;
                }
                TemplateElement::Image { src, style } => {
                    let rendered_src: Cow<'a, str> = if src.contains("{{") {
                        Cow::Owned(self.template_engine.render_template(src, context)
                            .map_err(|e| PipelineError::TemplateParseError(format!("Failed to render image src '{}': {}", src, e)))?)
                    } else {
                        Cow::Borrowed(src)
                    };
                    proxy.process_event(IDFEvent::AddImage {
                        src: rendered_src,
                        style: style.as_deref().map(Cow::Borrowed),
                        data: None,
                    }).await?;
                }
                TemplateElement::PageBreak => {
                    proxy.process_event(IDFEvent::ForcePageBreak).await?;
                }
            }
        }
        Ok(())
    }

    /// Handles the parsing of a `Table` template element.
    async fn parse_table(
        &mut self,
        data_source: &'a str,
        columns: &'a [TableColumn],
        style: &'a Option<String>,
        row_style_prefix_field: &'a Option<String>,
        context: &'a Value,
        proxy: &mut LayoutProcessorProxy<'a>,
    ) -> Result<(), PipelineError> {
        proxy.process_event(IDFEvent::StartTable {
            style: style.as_deref().map(Cow::Borrowed),
            columns: Cow::Borrowed(columns),
        }).await?;

        proxy.process_event(IDFEvent::StartHeader).await?;
        proxy.process_event(IDFEvent::StartRow {
            context: &Value::Null,
        }).await?;
        for (i, col) in columns.iter().enumerate() {
            proxy.process_event(IDFEvent::AddCell {
                column_index: i,
                content: Cow::Borrowed(&col.header),
                style_override: col.header_style.clone(),
            }).await?;
        }
        proxy.process_event(IDFEvent::EndRow).await?;
        proxy.process_event(IDFEvent::EndHeader).await?;

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
            let prefix_str = row_style_prefix_field
                .as_ref()
                .and_then(|field| row_item.pointer(field).and_then(|v| v.as_str()));

            proxy.process_event(IDFEvent::StartRow { context: row_item }).await?;

            for (i, col) in columns.iter().enumerate() {
                let cell_value = row_item.pointer(&col.data_field).unwrap_or(&Value::Null);

                let cell_text: Cow<'a, str> = if let Some(template_string) = &col.content_template {
                    Cow::Owned(self.template_engine.render_template(template_string, row_item).map_err(|e| {
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

                let final_style =
                    if let (Some(prefix), Some(base_style)) = (prefix_str, &col.style) {
                        Some(format!("{}-{}", prefix, base_style))
                    } else {
                        col.style.clone()
                    };

                proxy.process_event(IDFEvent::AddCell {
                    column_index: i,
                    content: cell_text,
                    style_override: final_style,
                }).await?;
            }
            proxy.process_event(IDFEvent::EndRow).await?;
        }

        proxy.process_event(IDFEvent::EndTable).await?;
        Ok(())
    }
}

#[async_trait(?Send)]
impl<'a> TemplateProcessor<'a> for JsonTemplateParser<'a> {
    async fn process(
        &mut self,
        data: &'a Value,
        proxy: &mut LayoutProcessorProxy<'a>,
    ) -> Result<(), PipelineError> {
        proxy.process_event(IDFEvent::StartDocument).await?;

        if self.stylesheet.page_sequences.is_empty() {
            return Err(PipelineError::StylesheetError(
                "No page_sequences defined in stylesheet.".to_string(),
            ));
        }

        for (_seq_name, sequence) in &self.stylesheet.page_sequences {
            let template: &Template = self
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
                proxy
                    .process_event(IDFEvent::BeginPageSequence { context: item_data })
                    .await?;
                self.parse_children(&template.children, item_data, proxy)
                    .await?;
                proxy.process_event(IDFEvent::EndPageSequence).await?;
            }
        }

        proxy.process_event(IDFEvent::EndDocument).await?;
        Ok(())
    }
}