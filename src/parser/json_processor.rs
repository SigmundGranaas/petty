use super::processor::TemplateProcessor;
use super::Event;
use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::render::DocumentRenderer;
use crate::stylesheet::{Stylesheet, TableColumn, Template, TemplateElement};
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
    fn parse_children<R: DocumentRenderer<'a>>(
        &mut self,
        children: &'a [TemplateElement],
        context: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
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
                    processor.process_event(Event::AddText {
                        content: rendered_content,
                        style: style.as_deref().map(Cow::Borrowed),
                    })?;
                }
                TemplateElement::Rectangle { style } => {
                    processor.process_event(Event::AddRectangle {
                        style: style.as_deref().map(Cow::Borrowed),
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
                            style: style.as_deref().map(Cow::Borrowed),
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
                TemplateElement::Image { src, style } => {
                    let rendered_src: Cow<'a, str> = if src.contains("{{") {
                        Cow::Owned(self.template_engine.render_template(src, context)
                            .map_err(|e| PipelineError::TemplateParseError(format!("Failed to render image src '{}': {}", src, e)))?)
                    } else {
                        Cow::Borrowed(src)
                    };
                    processor.process_event(Event::AddImage {
                        src: rendered_src,
                        style: style.as_deref().map(Cow::Borrowed),
                    })?;
                }
                TemplateElement::PageBreak => {
                    processor.process_event(Event::ForcePageBreak)?;
                }
            }
        }
        Ok(())
    }

    /// Handles the parsing of a `Table` template element.
    fn parse_table<R: DocumentRenderer<'a>>(
        &mut self,
        data_source: &'a str,
        columns: &'a [TableColumn],
        style: &'a Option<String>,
        row_style_prefix_field: &'a Option<String>,
        context: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError> {
        processor.process_event(Event::StartTable {
            style: style.as_deref().map(Cow::Borrowed),
            columns: Cow::Borrowed(columns),
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
            let prefix_str = row_style_prefix_field
                .as_ref()
                .and_then(|field| row_item.pointer(field).and_then(|v| v.as_str()));

            processor.process_event(Event::StartRow {
                context: row_item,
                row_style_prefix: prefix_str.map(String::from),
            })?;

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

impl<'a> TemplateProcessor<'a> for JsonTemplateParser<'a> {
    fn process<R: DocumentRenderer<'a>>(
        &mut self,
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
                processor.process_event(Event::BeginPageSequenceItem { context: item_data })?;
                self.parse_children(&template.children, item_data, processor)?;
                processor.process_event(Event::EndPageSequenceItem)?;
            }
        }

        processor.process_event(Event::EndDocument)?;
        Ok(())
    }
}