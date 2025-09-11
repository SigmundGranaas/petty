// src/layout/processor/mod.rs
mod container;
mod page;
mod primitives;
mod table;

use crate::error::PipelineError;
use crate::idf::{FlexDirection, IDFEvent};
use crate::layout::engine::LayoutEngine;
use crate::layout::style::ComputedStyle;
use crate::render::DocumentRenderer;
use crate::stylesheet::{PageLayout, Stylesheet, TableColumn};
use serde_json::Value;
use std::borrow::Cow;

#[derive(Clone)]
pub(super) enum LayoutType {
    Block,
    Flex(FlexDirection),
    ListItemBody,
}

pub(super) struct LayoutContext {
    layout_type: LayoutType,
    style: ComputedStyle,
    available_width: f32,
    content_x: f32,
    // Flex-specific state
    current_flex_x: f32,
    current_flex_line_height: f32,
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

    // NEW: For inline style overrides like <b> or <i>
    inline_style_stack: Vec<ComputedStyle>,

    // Table specific state
    current_table: Option<CurrentTable<'a>>,
    current_table_headers: Option<Vec<IDFEvent<'a>>>,
    is_in_header: bool,
    is_in_row: bool, // New flag
    is_in_cell: bool, // New flag
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
            inline_style_stack: Vec::new(),
            current_table: None,
            current_table_headers: None,
            is_in_header: false,
            is_in_row: false, // Initialize new flag
            is_in_cell: false, // Initialize new flag
            current_row_events: Vec::new(),
            current_row_context: None,
            current_page_sequence_context: None,
        }
    }

    pub fn into_renderer(self) -> R {
        self.renderer
    }

    pub fn process_event(&mut self, event: IDFEvent<'a>) -> Result<(), PipelineError> {
        // Handle header collection first
        if self.is_in_header {
            if let IDFEvent::EndHeader = event {
                self.is_in_header = false;
            } else if let Some(headers) = &mut self.current_table_headers {
                headers.push(event.clone()); // Collect header events
                return Ok(()); // Don't process header content events twice
            }
        }

        // If currently in a table row, collect all events until EndRow
        if self.is_in_row {
            match event {
                IDFEvent::EndRow => {
                    self.handle_end_row()?;
                    self.is_in_row = false;
                    self.current_row_context = None;
                    self.is_in_cell = false; // Ensure cell flag is reset
                }
                IDFEvent::StartCell { .. } => {
                    self.is_in_cell = true;
                    self.current_row_events.push(event);
                }
                IDFEvent::EndCell => {
                    self.is_in_cell = false;
                    self.current_row_events.push(event);
                }
                // All other events are collected if within a row (and potentially within a cell)
                _ => {
                    self.current_row_events.push(event);
                }
            }
            return Ok(()); // Event was collected for later row processing
        }

        // Process events normally if not in a table header or row
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
                if self.current_y > self.page_layout.margins.top {
                    self.start_new_page()?;
                }
            }

            // Table Events - only StartTable, EndTable, StartHeader, StartRow are handled directly here.
            // Other table-related events (StartCell, EndCell, EndRow) are handled via `is_in_row` logic above.
            IDFEvent::StartTable { style, columns } => self.handle_start_table(style, columns)?,
            IDFEvent::EndTable => self.handle_end_table()?,
            IDFEvent::StartHeader => {
                self.is_in_header = true;
                self.current_table_headers = Some(Vec::new());
            }
            IDFEvent::EndHeader => {
                self.is_in_header = false; // This is actually set in the conditional block above now.
            }
            IDFEvent::StartRow { context, .. } => {
                self.current_row_context = Some(context);
                self.current_row_events.clear(); // Clear for a new row
                self.is_in_row = true; // Set the flag to start collecting row events
            }
            // IDFEvent::AddCell { .. } => self.current_row_events.push(event), // REMOVED (no longer exists)
            IDFEvent::EndRow => {
                // This branch should ideally not be hit if `is_in_row` logic is correct,
                // as `EndRow` would be handled there.
                unreachable!("EndRow should be handled by `is_in_row` block.");
            }

            // --- NEW EVENTS ---
            IDFEvent::StartInline { style } => self.handle_start_inline(style)?,
            IDFEvent::EndInline => self.handle_end_inline()?,
            IDFEvent::AddLineBreak => self.handle_add_linebreak()?,
            IDFEvent::AddHyperlink { href, style } => self.handle_start_hyperlink(href, style)?,
            IDFEvent::EndHyperlink => self.handle_end_hyperlink()?,
            IDFEvent::StartFlexContainer { style, direction } => {
                self.handle_start_flex_container(style, direction)?
            }
            IDFEvent::EndFlexContainer => self.handle_end_flex_container()?,
            // List events are placeholders for now
            IDFEvent::StartList { style } => self.handle_start_container(style)?,
            IDFEvent::EndList => self.handle_end_container()?,
            IDFEvent::StartListItem => { /* Complex logic TBD */ }
            IDFEvent::EndListItem => { /* Complex logic TBD */ }
            IDFEvent::AddListItemLabel { .. } => { /* Complex logic TBD */ }
            IDFEvent::AddListItemBody => { /* Complex logic TBD */ }
            // These events should only be present if `is_in_row` is true, otherwise it's an error in parsing or logic.
            // If they are not handled by the `is_in_row` block, it means they appeared out of context.
            IDFEvent::StartCell { .. } | IDFEvent::EndCell => {
                return Err(PipelineError::TemplateParseError(
                    "StartCell/EndCell event outside of a table row context.".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn handle_start_inline(
        &mut self,
        style_name: Option<Cow<'a, str>>,
    ) -> Result<(), PipelineError> {
        let parent_style = self
            .inline_style_stack
            .last()
            .or_else(|| self.context_stack.last().map(|c| &c.style))
            .cloned()
            .unwrap_or_default();

        let new_style = self
            .layout_engine
            .compute_style_from_parent(style_name.as_deref(), &parent_style);
        self.inline_style_stack.push(new_style);
        Ok(())
    }

    fn handle_end_inline(&mut self) -> Result<(), PipelineError> {
        self.inline_style_stack.pop();
        Ok(())
    }

    fn handle_add_linebreak(&mut self) -> Result<(), PipelineError> {
        let current_style = self
            .context_stack
            .last()
            .map(|c| &c.style)
            .cloned()
            .unwrap_or_default();
        self.current_y += current_style.line_height;
        Ok(())
    }

    fn handle_start_hyperlink(
        &mut self,
        href: Cow<'a, str>,
        style: Option<Cow<'a, str>>,
    ) -> Result<(), PipelineError> {
        self.renderer.start_hyperlink(&href);
        self.handle_start_inline(style)
    }

    fn handle_end_hyperlink(&mut self) -> Result<(), PipelineError> {
        self.renderer.end_hyperlink();
        self.handle_end_inline()
    }

    fn handle_start_flex_container(
        &mut self,
        style: Option<Cow<'a, str>>,
        direction: FlexDirection,
    ) -> Result<(), PipelineError> {
        let (parent_style, parent_available_width, parent_content_x) =
            if let Some(parent_context) = self.context_stack.last() {
                (
                    parent_context.style.clone(),
                    parent_context.available_width,
                    parent_context.content_x,
                )
            } else {
                return Err(PipelineError::TemplateParseError(
                    "Flex container outside page sequence".to_string(),
                ));
            };

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
            layout_type: LayoutType::Flex(direction),
            style: new_style,
            available_width: new_available_width,
            content_x: new_content_x,
            current_flex_x: new_content_x,
            current_flex_line_height: 0.0,
        };
        self.context_stack.push(new_context);
        Ok(())
    }

    fn handle_end_flex_container(&mut self) -> Result<(), PipelineError> {
        let ended_context = self
            .context_stack
            .pop()
            .expect("EndFlexContainer without matching StartFlexContainer");
        self.current_y += ended_context.current_flex_line_height;
        self.current_y += ended_context.style.padding.bottom + ended_context.style.margin.bottom;
        Ok(())
    }
}