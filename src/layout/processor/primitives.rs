use super::StreamingLayoutProcessor;
use crate::error::PipelineError;
use crate::layout::elements::{LayoutElement, PositionedElement, RectElement, TextElement};
use crate::render::DocumentRenderer;
use std::borrow::Cow;

impl<'a, R: DocumentRenderer<'a>> StreamingLayoutProcessor<'a, R> {
    pub(super) fn handle_add_rectangle(&mut self, style_name: Option<&'a str>) -> Result<(), PipelineError> {
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

    pub(super) fn handle_add_text(
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
}