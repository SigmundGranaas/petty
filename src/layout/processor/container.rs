// src/layout/processor/container.rs
use super::{LayoutContext, LayoutType, StreamingLayoutProcessor};
use crate::error::PipelineError;
use crate::render::DocumentRenderer;
use std::borrow::Cow;

impl<'a, R: DocumentRenderer<'a>> StreamingLayoutProcessor<'a, R> {
    pub(super) fn handle_start_container(
        &mut self,
        style: Option<Cow<'a, str>>,
    ) -> Result<(), PipelineError> {
        let parent_context = self
            .context_stack
            .last()
            .expect("Page context should be guaranteed by process_event");

        let (parent_style, parent_available_width, parent_content_x) = (
            parent_context.style.clone(),
            parent_context.available_width,
            parent_context.content_x,
        );

        let new_style = self
            .layout_engine
            .compute_style_from_parent(style.as_deref(), &parent_style);

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

        let new_context = LayoutContext {
            layout_type: LayoutType::Block,
            style: new_style,
            available_width: new_available_width,
            content_x: new_content_x,
            current_flex_x: new_content_x,
            current_flex_line_height: 0.0,
        };
        self.context_stack.push(new_context);
        Ok(())
    }

    pub(super) fn handle_end_container(&mut self) -> Result<(), PipelineError> {
        let ended_context = self
            .context_stack
            .pop()
            .expect("EndContainer called without a matching StartContainer.");
        self.current_y += ended_context.style.padding.bottom + ended_context.style.margin.bottom;
        Ok(())
    }
}