// FILE: /home/sigmund/RustroverProjects/petty/src/layout/processor.rs
// src/layout/processor.rs

use crate::error::PipelineError;
use crate::layout::model::{
    ComputedStyle, LayoutElement, LayoutEngine, PositionedElement, RectElement, TextElement,
};
use crate::parser::Event;
use crate::render::DocumentRenderer;
use crate::stylesheet::{Dimension, PageLayout, Stylesheet, TableColumn};
use serde_json::Value;
use std::borrow::Cow;

struct LayoutContext {
    style: ComputedStyle,
    available_width: f32,
    content_x: f32,
}

struct CurrentTable<'a> {
    columns: &'a [TableColumn],
    column_widths: Vec<f32>,
    style: ComputedStyle,
}

pub struct StreamingLayoutProcessor<'a, R: DocumentRenderer<'a>> {
    renderer: R,
    page_layout: PageLayout,
    layout_engine: LayoutEngine,
    current_y: f32,
    page_height: f32,
    context_stack: Vec<LayoutContext>,

    // Table specific state
    current_table: Option<CurrentTable<'a>>,
    current_table_headers: Option<Vec<Event<'a>>>,
    is_in_header: bool,
    current_row_events: Vec<Event<'a>>,
    current_row_context: Option<&'a serde_json::Value>,
    // Field to track the context for the current logical page (record).
    current_page_sequence_context: Option<&'a serde_json::Value>,
}

impl<'a, R: DocumentRenderer<'a>> StreamingLayoutProcessor<'a, R> {
    pub fn new(renderer: R, stylesheet: &'a Stylesheet) -> Self {
        let layout_engine = LayoutEngine::new(stylesheet);
        StreamingLayoutProcessor {
            renderer,
            page_layout: stylesheet.page.clone(),
            layout_engine,
            current_y: 0.0,
            page_height: 0.0,
            context_stack: Vec::new(),
            current_table: None,
            current_table_headers: None,
            is_in_header: false,
            current_row_events: Vec::new(),
            current_row_context: None,
            current_page_sequence_context: None,
        }
    }

    pub fn into_renderer(self) -> R {
        self.renderer
    }

    pub fn process_event(&mut self, event: Event<'a>) -> Result<(), PipelineError> {
        if self.is_in_header {
            if let Event::EndHeader = event {
                // Don't store the EndHeader event itself
            } else if let Some(headers) = &mut self.current_table_headers {
                headers.push(event.clone());
            }
        }

        match event {
            Event::StartDocument => {
                self.renderer.begin_document()?;
            }
            Event::EndDocument => {
                // Finalization happens in the pipeline
            }
            Event::BeginPageSequenceItem { context } => {
                self.current_page_sequence_context = Some(context);
                self.start_new_page()?;
            }
            Event::EndPageSequenceItem => {}
            Event::StartContainer { style } => self.handle_start_container(style)?,
            Event::EndContainer => self.handle_end_container()?,
            Event::AddText { content, style } => self.handle_add_text(&content, style)?,
            Event::AddRectangle { style } => self.handle_add_rectangle(style)?,
            Event::ForcePageBreak => self.start_new_page()?,

            // Table Events
            Event::StartTable { style, columns } => self.handle_start_table(style, columns)?,
            Event::EndTable => self.handle_end_table()?,
            Event::StartHeader => {
                self.is_in_header = true;
                self.current_table_headers = Some(Vec::new());
            }
            Event::EndHeader => {
                self.is_in_header = false;
            }
            Event::StartRow { context, .. } => {
                self.current_row_context = Some(context);
            }
            Event::AddCell { .. } => self.current_row_events.push(event),
            Event::EndRow => self.handle_end_row()?,
        }
        Ok(())
    }

    fn start_new_page(&mut self) -> Result<(), PipelineError> {
        if let Some(context) = self.current_page_sequence_context {
            self.renderer.start_new_logical_page(context);
        } else {
            self.renderer.start_new_logical_page(&Value::Null);
        }

        self.renderer.begin_page(&self.page_layout)?;
        let page_dims = self.layout_engine.get_page_dimensions();
        self.page_height = page_dims.1;
        let available_width =
            page_dims.0 - self.page_layout.margins.left - self.page_layout.margins.right;
        self.current_y = self.page_layout.margins.top;
        self.context_stack.clear();
        self.context_stack.push(LayoutContext {
            style: self.layout_engine.compute_style_from_default(None),
            available_width,
            content_x: self.page_layout.margins.left,
        });
        Ok(())
    }

    fn needs_page_break(&self, required_height: f32) -> bool {
        let available_space = self.page_height
            - self.current_y
            - self.page_layout.margins.bottom
            - self.page_layout.footer_height;
        self.current_y > self.page_layout.margins.top && available_space < required_height
    }

    fn handle_start_container(&mut self, style: Option<&'a str>) -> Result<(), PipelineError> {
        let (parent_style, parent_available_width, parent_content_x) = {
            let parent_context = self.context_stack.last().unwrap();
            (
                parent_context.style.clone(),
                parent_context.available_width,
                parent_context.content_x,
            )
        };
        let new_style = self
            .layout_engine
            .compute_style_from_parent(style, &parent_style);

        if self.needs_page_break(new_style.margin.top) {
            self.start_new_page()?;
        }
        self.current_y += new_style.margin.top;

        let new_available_width = parent_available_width
            - new_style.margin.left
            - new_style.margin.right
            - new_style.padding.left
            - new_style.padding.right;

        let new_content_x = parent_content_x + new_style.margin.left + new_style.padding.left;

        self.current_y += new_style.padding.top;

        self.context_stack.push(LayoutContext {
            style: new_style,
            available_width: new_available_width,
            content_x: new_content_x,
        });
        Ok(())
    }

    fn handle_end_container(&mut self) -> Result<(), PipelineError> {
        let ended_context = self
            .context_stack
            .pop()
            .expect("EndContainer called without a matching StartContainer.");
        self.current_y += ended_context.style.padding.bottom + ended_context.style.margin.bottom;
        Ok(())
    }

    fn handle_add_rectangle(&mut self, style_name: Option<&'a str>) -> Result<(), PipelineError> {
        let (parent_style, parent_available_width, parent_content_x) = {
            let parent_context = self.context_stack.last().unwrap();
            (
                parent_context.style.clone(),
                parent_context.available_width,
                parent_context.content_x,
            )
        };
        let style = self
            .layout_engine
            .compute_style_from_parent(style_name, &parent_style);

        let height = style.height.unwrap_or(2.0);
        let required_space = style.margin.top + height + style.margin.bottom;
        if self.needs_page_break(required_space) {
            self.start_new_page()?;
        }

        let y = self.current_y + style.margin.top;
        let positioned = PositionedElement {
            x: parent_content_x + style.margin.left,
            y,
            width: parent_available_width - style.margin.left - style.margin.right,
            height,
            element: LayoutElement::Rectangle(RectElement {
                style_name: style_name.map(String::from),
            }),
            style,
        };
        self.renderer
            .render_element(&positioned, &self.layout_engine)?;
        self.current_y = y + height + positioned.style.margin.bottom;
        Ok(())
    }

    fn handle_add_text(
        &mut self,
        content: &Cow<'a, str>,
        style_name: Option<&'a str>,
    ) -> Result<(), PipelineError> {
        let (parent_style, parent_available_width, parent_content_x) = {
            let parent_context = self.context_stack.last().unwrap();
            (
                parent_context.style.clone(),
                parent_context.available_width,
                parent_context.content_x,
            )
        };

        let style = self
            .layout_engine
            .compute_style_from_parent(style_name, &parent_style);

        let content_width = parent_available_width
            - style.margin.left
            - style.margin.right
            - style.padding.left
            - style.padding.right;

        let lines = self
            .layout_engine
            .wrap_text(content.as_ref(), &style, content_width);
        let mut line_cursor = 0;

        while line_cursor < lines.len() {
            let required_space_for_first_line = style.margin.top
                + style.padding.top
                + style.line_height
                + style.padding.bottom
                + style.margin.bottom;
            if self.needs_page_break(required_space_for_first_line) {
                self.start_new_page()?;
            }

            let y_after_margin = self.current_y + style.margin.top;
            let available_space = self.page_height
                - y_after_margin
                - self.page_layout.margins.bottom
                - self.page_layout.footer_height;

            let space_for_lines = available_space - style.padding.top - style.padding.bottom;
            let lines_that_fit = (space_for_lines / style.line_height).floor() as usize;
            let num_lines_to_draw = std::cmp::min(lines.len() - line_cursor, lines_that_fit.max(1));

            if num_lines_to_draw == 0 && line_cursor < lines.len() {
                self.start_new_page()?;
                continue;
            }

            let chunk_of_lines = &lines[line_cursor..line_cursor + num_lines_to_draw];
            let text_block_height = chunk_of_lines.len() as f32 * style.line_height;
            let total_height = text_block_height + style.padding.top + style.padding.bottom;

            let positioned = PositionedElement {
                x: parent_content_x + style.margin.left,
                y: y_after_margin,
                width: content_width + style.padding.left + style.padding.right,
                height: total_height,
                element: LayoutElement::Text(TextElement {
                    style_name: style_name.map(String::from),
                    content: chunk_of_lines.join("\n"),
                }),
                style: style.clone(),
            };

            self.renderer
                .render_element(&positioned, &self.layout_engine)?;
            self.current_y = y_after_margin + total_height + style.margin.bottom;
            line_cursor += num_lines_to_draw;
        }
        Ok(())
    }

    // Table specific methods
    fn handle_start_table(
        &mut self,
        style_name: Option<&'a str>,
        columns: &'a [TableColumn],
    ) -> Result<(), PipelineError> {
        let parent_context = self.context_stack.last().unwrap();
        let style = self
            .layout_engine
            .compute_style_from_parent(style_name, &parent_context.style);

        let table_width = parent_context.available_width - style.margin.left - style.margin.right;
        let column_widths = self.calculate_column_widths(columns, table_width);

        self.current_table = Some(CurrentTable {
            columns,
            column_widths,
            style,
        });
        Ok(())
    }

    fn calculate_column_widths(&self, columns: &[TableColumn], available_width: f32) -> Vec<f32> {
        let mut widths = vec![0.0; columns.len()];
        let mut remaining_width = available_width;
        let mut percent_total = 0.0;

        for (i, col) in columns.iter().enumerate() {
            if let Some(dim) = &col.width {
                match dim {
                    Dimension::Pt(w) => {
                        widths[i] = *w;
                        remaining_width -= *w;
                    }
                    Dimension::Percent(p) => percent_total += p,
                    _ => {}
                }
            }
        }

        if percent_total > 0.0 {
            let width_for_percent = remaining_width;
            for (i, col) in columns.iter().enumerate() {
                if let Some(Dimension::Percent(p)) = &col.width {
                    let new_width = (p / 100.0) * width_for_percent;
                    widths[i] = new_width;
                    remaining_width -= new_width;
                }
            }
        }

        let auto_cols: Vec<usize> = widths
            .iter()
            .enumerate()
            .filter(|(_, &w)| w == 0.0)
            .map(|(i, _)| i)
            .collect();
        if !auto_cols.is_empty() {
            let width_per_auto = remaining_width / auto_cols.len() as f32;
            for i in auto_cols {
                widths[i] = width_per_auto;
            }
        }

        widths
    }

    fn handle_end_table(&mut self) -> Result<(), PipelineError> {
        self.current_table = None;
        Ok(())
    }

    fn handle_end_row(&mut self) -> Result<(), PipelineError> {
        // Take ownership of the events to avoid borrow-checker conflicts later.
        let current_row_events = std::mem::take(&mut self.current_row_events);
        let row_height = self.calculate_row_height_from_events(&current_row_events)?;

        if self.needs_page_break(row_height) {
            self.start_new_page()?;
            // Re-render headers on the new page.
            // Temporarily take the headers to avoid borrow conflicts, which prevents a costly clone.
            if let Some(headers) = self.current_table_headers.take() {
                let header_height = self.calculate_row_height_from_events(&headers)?;
                self.render_row(&headers, header_height)?;
                // After rendering, put the headers back for subsequent pages.
                self.current_table_headers = Some(headers);
            }
        }

        // Render the current row using the owned events.
        self.render_row(&current_row_events, row_height)?;
        Ok(())
    }

    fn calculate_row_height_from_events(
        &self,
        row_events: &[Event<'a>],
    ) -> Result<f32, PipelineError> {
        let table = self.current_table.as_ref().unwrap();
        let mut max_height = 0.0f32;

        for event in row_events {
            if let Event::AddCell {
                column_index,
                content,
                style_override,
            } = event
            {
                let style = self.layout_engine.compute_style_from_parent(
                    style_override.as_deref(),
                    &table.style,
                );
                let col_width = table.column_widths[*column_index];
                let content_width = col_width - style.padding.left - style.padding.right;
                let lines = self
                    .layout_engine
                    .wrap_text(content.as_ref(), &style, content_width);
                let cell_height = (lines.len() as f32 * style.line_height)
                    + style.padding.top
                    + style.padding.bottom;
                max_height = max_height.max(cell_height);
            }
        }
        Ok(max_height)
    }

    fn render_row(&mut self, row_events: &[Event<'a>], row_height: f32) -> Result<(), PipelineError> {
        let table = self.current_table.as_ref().unwrap();
        let parent_context = self.context_stack.last().unwrap();
        let table_start_x = parent_context.content_x;

        for event in row_events {
            if let Event::AddCell {
                column_index,
                content,
                style_override,
            } = event
            {
                // Calculate position for this cell explicitly instead of accumulating.
                // This is more robust.
                let col_offset: f32 = table.column_widths.iter().take(*column_index).sum();
                let cell_x = table_start_x + col_offset;
                let col_width = table.column_widths[*column_index];

                let style = self.layout_engine.compute_style_from_parent(
                    style_override.as_deref(),
                    &table.style,
                );

                let cell_bg = PositionedElement {
                    x: cell_x,
                    y: self.current_y,
                    width: col_width,
                    height: row_height,
                    element: LayoutElement::Rectangle(RectElement {
                        style_name: style_override.clone(),
                    }),
                    style: style.clone(),
                };
                self.renderer.render_element(&cell_bg, &self.layout_engine)?;

                let cell_text = PositionedElement {
                    x: cell_x,
                    y: self.current_y,
                    width: col_width,
                    height: row_height,
                    element: LayoutElement::Text(TextElement {
                        style_name: style_override.clone(),
                        content: content.to_string(),
                    }),
                    style,
                };
                self.renderer
                    .render_element(&cell_text, &self.layout_engine)?;
            }
        }
        self.current_y += row_height;
        Ok(())
    }
}