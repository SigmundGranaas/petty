mod container;
mod page;
mod primitives;
mod table;

use crate::error::PipelineError;
use crate::layout::engine::LayoutEngine;
use crate::layout::style::ComputedStyle;
use crate::parser::Event;
use crate::render::DocumentRenderer;
use crate::stylesheet::{PageLayout, Stylesheet, TableColumn};

pub(super) struct LayoutContext {
    style: ComputedStyle,
    available_width: f32,
    content_x: f32,
}

pub(super) struct CurrentTable<'a> {
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
}