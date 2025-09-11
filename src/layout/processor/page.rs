// src/layout/processor/page.rs
use super::{LayoutContext, LayoutType, StreamingLayoutProcessor};
use crate::error::PipelineError;
use crate::render::DocumentRenderer;
use serde_json::Value;

impl<'a, R: DocumentRenderer<'a>> StreamingLayoutProcessor<'a, R> {
    pub(super) fn start_new_page(&mut self) -> Result<(), PipelineError> {
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
        let root_context = LayoutContext {
            layout_type: LayoutType::Block,
            style: self.layout_engine.compute_style_from_default(None),
            available_width,
            content_x: self.page_layout.margins.left,
            current_flex_x: self.page_layout.margins.left,
            current_flex_line_height: 0.0,
        };
        self.context_stack.push(root_context);
        Ok(())
    }

    pub(super) fn needs_page_break(&self, required_height: f32) -> bool {
        let available_space = self.page_height
            - self.current_y
            - self.page_layout.margins.bottom
            - self.page_layout.footer_height;
        self.current_y > self.page_layout.margins.top && available_space < required_height
    }
}