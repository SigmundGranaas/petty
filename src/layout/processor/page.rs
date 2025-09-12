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

        // Take the old stack of open containers. This is crucial for page breaks
        // that happen inside nested elements.
        let old_stack = std::mem::take(&mut self.context_stack);

        // Establish a new root context for the new page.
        let root_context = LayoutContext {
            layout_type: LayoutType::Block,
            style: self.layout_engine.compute_style_from_default(None),
            available_width,
            content_x: self.page_layout.margins.left,
            current_flex_x: self.page_layout.margins.left,
            current_flex_line_height: 0.0,
        };
        self.context_stack.push(root_context);

        // Now, "replay" the stack of open containers on the new page.
        // We skip the first element of the old stack, which was the old page root.
        // This ensures the layout engine's state remains synchronized with the parser's state.
        for container_context in old_stack.into_iter().skip(1) {
            let parent_context = self.context_stack.last().unwrap(); // Safe due to push above

            // We reuse the style and layout type of the container from the previous page.
            let style = container_context.style;

            // But we must recalculate its geometry based on the new parent (the new root,
            // or the replayed container above it).
            let new_available_width = parent_context.available_width
                - style.margin.left
                - style.margin.right
                - style.padding.left
                - style.padding.right;

            let new_content_x = parent_context.content_x + style.margin.left + style.padding.left;

            // We don't add margin/padding to current_y, as we assume the container is "continuing"
            // at the top of the new page. Padding will be respected by child elements.

            let replayed_context = LayoutContext {
                layout_type: container_context.layout_type,
                style,
                available_width: new_available_width,
                content_x: new_content_x,
                current_flex_x: new_content_x, // Reset flex cursor for the new page
                current_flex_line_height: 0.0, // Reset flex line height for the new page
            };
            self.context_stack.push(replayed_context);
        }

        Ok(())
    }

    pub(super) fn needs_page_break(&self, required_height: f32) -> bool {
        let available_space = self.page_height
            - self.current_y
            - self.page_layout.margins.bottom
            - self.page_layout.footer_height;
        // A page break is needed if we are not at the very top of the page AND
        // there isn't enough space for the element.
        self.current_y > self.page_layout.margins.top && available_space < required_height
    }
}