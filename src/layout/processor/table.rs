// src/layout/processor/table.rs
use super::{CurrentTable, LayoutContext, LayoutType, StreamingLayoutProcessor};
use crate::error::PipelineError;
use crate::idf::IDFEvent;
use crate::layout::elements::{LayoutElement, PositionedElement, RectElement, TextElement};
use crate::render::DocumentRenderer;
use crate::stylesheet::{Dimension, TableColumn};
use std::borrow::Cow;
use crate::layout::ImageElement;

impl<'a, R: DocumentRenderer<'a>> StreamingLayoutProcessor<'a, R> {
    pub(super) fn handle_start_table(
        &mut self,
        style_name: Option<Cow<'a, str>>,
        columns: Cow<'a, [TableColumn]>,
    ) -> Result<(), PipelineError> {
        let parent_context = self
            .context_stack
            .last()
            .expect("Page context should be guaranteed by process_event");

        let style = self
            .layout_engine
            .compute_style_from_parent(style_name.as_deref(), &parent_context.style);

        let table_width = parent_context.available_width - style.margin.left - style.margin.right;
        let column_widths = self.calculate_column_widths(&columns, table_width);

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
        row_events: &[IDFEvent<'a>],
    ) -> Result<f32, PipelineError> {
        let table = self.current_table.as_ref().unwrap();
        let mut max_cell_height_in_row = 0.0f32;

        let mut current_cell_column_index: usize = 0;
        let mut current_cell_style_override: Option<String> = None;
        let mut cell_content_events_start_index = 0;

        for (i, event) in row_events.iter().enumerate() {
            match event {
                IDFEvent::StartCell {
                    column_index,
                    style_override,
                } => {
                    current_cell_column_index = *column_index;
                    current_cell_style_override = style_override.clone();
                    cell_content_events_start_index = i + 1;
                }
                IDFEvent::EndCell => {
                    let cell_content_events = &row_events[cell_content_events_start_index..i];

                    let col_width = table.column_widths[current_cell_column_index];
                    let cell_style = self.layout_engine.compute_style_from_parent(
                        current_cell_style_override.as_deref(),
                        &table.style,
                    );
                    let cell_content_width = col_width
                        - cell_style.padding.left
                        - cell_style.padding.right;

                    let mut current_y_in_cell = cell_style.padding.top; // Relative Y within the cell

                    for content_event in cell_content_events {
                        if let IDFEvent::AddText {
                            content,
                            style: text_style_name,
                        } = content_event
                        {
                            let text_style = self.layout_engine.compute_style_from_parent(
                                text_style_name.as_deref(),
                                &cell_style, // Parent style for text is the cell's container style
                            );
                            let lines = self.layout_engine.wrap_text(
                                content.as_ref(),
                                &text_style,
                                cell_content_width,
                            );
                            let text_content_height = lines.len() as f32 * text_style.line_height;
                            let text_element_height = text_content_height
                                + text_style.padding.top
                                + text_style.padding.bottom;
                            current_y_in_cell += text_style.margin.top
                                + text_element_height
                                + text_style.margin.bottom;
                        }
                        // Handle other elements (e.g., images) that might contribute to cell height
                    }
                    let current_cell_content_height = current_y_in_cell + cell_style.padding.bottom;
                    max_cell_height_in_row =
                        max_cell_height_in_row.max(current_cell_content_height);
                }
                _ => {}
            }
        }
        Ok(max_cell_height_in_row)
    }

    fn render_row(
        &mut self,
        row_events: &[IDFEvent<'a>],
        row_height: f32,
    ) -> Result<(), PipelineError> {
        let table = self.current_table.as_ref().unwrap();
        let parent_context = self.context_stack.last().unwrap();
        let table_start_x = parent_context.content_x;

        let mut current_cell_x_offset: f32 = 0.0;
        let mut current_cell_column_index: usize = 0;
        let mut current_cell_style_override: Option<String> = None;
        let mut cell_content_events_start_index = 0;

        for (i, event) in row_events.iter().enumerate() {
            match event {
                IDFEvent::StartCell {
                    column_index,
                    style_override,
                } => {
                    current_cell_column_index = *column_index;
                    current_cell_style_override = style_override.clone();
                    cell_content_events_start_index = i + 1; // Content starts after this event

                    // Calculate X for the current cell
                    current_cell_x_offset = table
                        .column_widths
                        .iter()
                        .take(current_cell_column_index)
                        .sum();
                    let cell_x = table_start_x + current_cell_x_offset;
                    let col_width = table.column_widths[current_cell_column_index];

                    // Compute style for the cell itself (container style)
                    let cell_style = self.layout_engine.compute_style_from_parent(
                        current_cell_style_override.as_deref(),
                        &table.style,
                    );

                    // Render cell background and border
                    let cell_bg_element = PositionedElement {
                        x: cell_x,
                        y: self.current_y,
                        width: col_width,
                        height: row_height,
                        element: LayoutElement::Rectangle(RectElement {
                            style_name: current_cell_style_override.clone(),
                        }),
                        style: cell_style.clone(),
                    };
                    self.renderer
                        .render_element(&cell_bg_element, &self.layout_engine)?;
                }
                IDFEvent::EndCell => {
                    let cell_content_events = &row_events[cell_content_events_start_index..i];
                    let cell_x = table_start_x + current_cell_x_offset;
                    let col_width = table.column_widths[current_cell_column_index];

                    let cell_style = self.layout_engine.compute_style_from_parent(
                        current_cell_style_override.as_deref(),
                        &table.style,
                    );
                    let cell_content_width = col_width
                        - cell_style.padding.left
                        - cell_style.padding.right;

                    let mut current_y_in_cell = self.current_y + cell_style.padding.top;
                    let current_x_in_cell = cell_x + cell_style.padding.left;

                    // Render content events within the current cell
                    // Temporarily push a context for the cell content rendering
                    self.context_stack.push(LayoutContext {
                        layout_type: LayoutType::Block, // Cells are block containers for their content
                        style: cell_style.clone(), // Use cell's style as parent
                        available_width: cell_content_width,
                        content_x: current_x_in_cell,
                        current_flex_x: current_x_in_cell,
                        current_flex_line_height: 0.0,
                    });

                    // Iterate through the content events within the cell and render them
                    for content_event in cell_content_events {
                        match content_event {
                            IDFEvent::AddText { content, style: text_style_name } => {
                                let text_style = self.layout_engine.compute_style_from_parent(
                                    text_style_name.as_deref(),
                                    self.context_stack.last().map(|c| &c.style).unwrap_or(&self.layout_engine.compute_style_from_default(None)),
                                );

                                // Text wrapping and height calculation for rendering
                                let lines = self.layout_engine.wrap_text(content.as_ref(), &text_style, cell_content_width);
                                let text_content_height = lines.len() as f32 * text_style.line_height;
                                let text_element_height = text_content_height
                                    + text_style.padding.top
                                    + text_style.padding.bottom;

                                let positioned_text = PositionedElement {
                                    x: current_x_in_cell + text_style.margin.left,
                                    y: current_y_in_cell + text_style.margin.top,
                                    width: cell_content_width,
                                    height: text_element_height,
                                    element: LayoutElement::Text(TextElement {
                                        style_name: text_style_name.as_deref().map(String::from),
                                        content: content.to_string(),
                                    }),
                                    style: text_style.clone(),
                                };
                                self.renderer.render_element(&positioned_text, &self.layout_engine)?;
                                current_y_in_cell += text_style.margin.top
                                    + text_element_height
                                    + text_style.margin.bottom;
                            }
                            IDFEvent::AddImage { src, style, data } => {
                                // For simplicity, image rendering within a cell will not respect current_y_in_cell
                                // for its own height adjustments unless carefully managed.
                                // We'll delegate to the image handler and then manually adjust current_y_in_cell.
                                // This is a simplified implementation. A full implementation would involve
                                // pushing a new LayoutContext and using self.handle_add_image, then pop.
                                let compute_style = &self.layout_engine.compute_style_from_default(None);
                                let temp_parent_style = self.context_stack.last().map(|c| &c.style).unwrap_or(compute_style);
                                let img_style = self.layout_engine.compute_style_from_parent(style.as_deref(), temp_parent_style);

                                if let Some(raw_data) = data {
                                    if let Ok(image) = image::load_from_memory(raw_data) {
                                        let (img_w, img_h) = (image.width(), image.height());
                                        let (width, height) = match (img_style.width, img_style.height) {
                                            (Some(w), Some(h)) => (w, h),
                                            (Some(w), None) => { let aspect_ratio = img_h as f32 / img_w as f32; (w, w * aspect_ratio) },
                                            (None, Some(h)) => { let aspect_ratio = img_w as f32 / img_h as f32; (h * aspect_ratio, h) },
                                            (None, None) => (img_w as f32, img_h as f32), // use intrinsic size
                                        };

                                        let positioned_image = PositionedElement {
                                            x: current_x_in_cell + img_style.margin.left,
                                            y: current_y_in_cell + img_style.margin.top,
                                            width,
                                            height,
                                            element: LayoutElement::Image(ImageElement { src: src.to_string(), image_data: raw_data.clone() }),
                                            style: img_style.clone(),
                                        };
                                        self.renderer.render_element(&positioned_image, &self.layout_engine)?;
                                        current_y_in_cell += height + img_style.margin.bottom + img_style.margin.top;
                                    }
                                }
                            }
                            // Add other content types here
                            _ => {}
                        }
                    }
                    self.context_stack.pop(); // Pop the temporary cell content context
                }
                _ => {}
            }
        }
        self.current_y += row_height;
        Ok(())
    }
}