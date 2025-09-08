use super::{Event, Parser};
use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::render::DocumentRenderer;
use crate::stylesheet::TableColumn;
use serde_json::Value;
use std::borrow::Cow;

impl<'a> Parser<'a> {
    /// Handles the parsing of a `Table` template element, generating all
    /// the necessary table-related events.
    pub(super) fn parse_table<R: DocumentRenderer<'a>>(
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

        // --- Header Row ---
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

        // --- Data Rows ---
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