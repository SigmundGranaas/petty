// src/layout/processor/mod.rs
mod container;
mod page;
mod primitives;
mod table;

use crate::error::PipelineError;
use crate::idf::IDFEvent;
use crate::layout::engine::LayoutEngine;
use crate::layout::style::ComputedStyle;
use crate::render::DocumentRenderer;
use crate::stylesheet::{PageLayout, Stylesheet, TableColumn};
use serde_json::Value;
use std::borrow::Cow;

pub(super) struct LayoutContext {
    style: ComputedStyle,
    available_width: f32,
    content_x: f32,
}

pub(super) struct CurrentTable<'a> {
    columns: Cow<'a, [TableColumn]>,
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
    current_table_headers: Option<Vec<IDFEvent<'a>>>,
    is_in_header: bool,
    current_row_events: Vec<IDFEvent<'a>>,
    current_row_context: Option<&'a Value>,
    // Field to track the context for the current logical page (record).
    current_page_sequence_context: Option<&'a Value>,
}

impl<'a, R: DocumentRenderer<'a>> StreamingLayoutProcessor<'a, R> {
    pub fn new(renderer: R, stylesheet: &'a Stylesheet) -> Self {
        StreamingLayoutProcessor {
            renderer,
            page_layout: stylesheet.page.clone(),
            layout_engine: LayoutEngine::new(stylesheet),
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

    pub fn process_event(&mut self, event: IDFEvent<'a>) -> Result<(), PipelineError> {
        if self.is_in_header {
            if let IDFEvent::EndHeader = event {
                // Don't store the EndHeader event itself
            } else if let Some(headers) = &mut self.current_table_headers {
                headers.push(event.clone());
            }
        }

        match event {
            IDFEvent::StartDocument => {
                self.renderer.begin_document()?;
            }
            IDFEvent::EndDocument => {
                // Finalization happens in the pipeline
            }
            IDFEvent::BeginPageSequence { context } => {
                self.current_page_sequence_context = Some(context);
                self.start_new_page()?;
            }
            IDFEvent::EndPageSequence => {}
            IDFEvent::StartBlock { style } => self.handle_start_container(style)?,
            IDFEvent::EndBlock => self.handle_end_container()?,
            IDFEvent::AddText { content, style } => self.handle_add_text(&content, style)?,
            IDFEvent::AddRectangle { style } => self.handle_add_rectangle(style)?,
            IDFEvent::AddImage { src, style, data } => self.handle_add_image(src, style, data)?,
            IDFEvent::ForcePageBreak => {
                // If we are already at the very top of a page, a page break does nothing.
                // This prevents creating a blank page if the first element is a page break.
                if self.current_y > self.page_layout.margins.top {
                    self.start_new_page()?;
                }
            }

            // Table Events
            IDFEvent::StartTable { style, columns } => self.handle_start_table(style, columns)?,
            IDFEvent::EndTable => self.handle_end_table()?,
            IDFEvent::StartHeader => {
                self.is_in_header = true;
                self.current_table_headers = Some(Vec::new());
            }
            IDFEvent::EndHeader => {
                self.is_in_header = false;
            }
            IDFEvent::StartRow { context, .. } => {
                self.current_row_context = Some(context);
            }
            IDFEvent::AddCell { .. } => self.current_row_events.push(event),
            IDFEvent::EndRow => self.handle_end_row()?,
        }
        Ok(())
    }
}