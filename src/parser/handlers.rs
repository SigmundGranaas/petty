use super::{Event, Parser};
use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::render::DocumentRenderer;
use crate::stylesheet::TemplateElement;
use serde_json::Value;
use std::borrow::Cow;

impl<'a> Parser<'a> {
    /// Recursively parses a list of template elements within a given data context,
    /// emitting events to the layout processor.
    pub(super) fn parse_children<R: DocumentRenderer<'a>>(
        &self,
        children: &'a [TemplateElement],
        context: &'a Value,
        processor: &mut StreamingLayoutProcessor<'a, R>,
    ) -> Result<(), PipelineError> {
        for element in children {
            match element {
                TemplateElement::Text { content, style, .. } => {
                    let rendered_content: Cow<'a, str> = if content.contains("{{") {
                        Cow::Owned(self.template_engine.render_template(content, context)
                            .map_err(|e| PipelineError::TemplateParseError(format!("Failed to render template '{}': {}", content, e)))?)
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
                    // Delegate to the specialized table parser
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
                // Silently ignore other element types like Image for now
                _ => {}
            }
        }
        Ok(())
    }
}