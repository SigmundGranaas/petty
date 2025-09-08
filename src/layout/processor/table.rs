use super::{CurrentTable, StreamingLayoutProcessor};
use crate::error::PipelineError;
use crate::layout::elements::{LayoutElement, PositionedElement, RectElement, TextElement};
use crate::parser::Event;
use crate::render::DocumentRenderer;
use crate::stylesheet::{Dimension, TableColumn};

impl<'a, R: DocumentRenderer<'a>> StreamingLayoutProcessor<'a, R> {
    pub(super) fn handle_start_table(
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

    pub(super) fn handle_end_table(&mut self) -> Result<(), PipelineError> {
        self.current_table = None;
        Ok(())
    }

    pub(super) fn handle_end_row(&mut self) -> Result<(), PipelineError> {
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