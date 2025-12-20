//! Handlers for table-related XSL-FO and custom table elements.

use crate::ast::{PreparsedTemplate, XsltInstruction};
use crate::compiler::{BuilderState, CompilerBuilder};
use crate::error::XsltError;
use crate::util::{OwnedAttributes, get_attr_owned_optional, get_line_col_from_pos};
use petty_style::parsers::{self as style_parsers, run_parser};

impl CompilerBuilder {
    pub(crate) fn handle_table_start(&mut self, attrs: OwnedAttributes) {
        self.state_stack.push(BuilderState::Table {
            attrs,
            columns: Vec::new(),
        });
    }

    pub(crate) fn handle_table_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        if let BuilderState::Table {
            attrs,
            columns: col_strings,
        } = current_state
        {
            let location = get_line_col_from_pos(source, pos).into();
            let mut header = None;
            let mut columns = Vec::new();

            if let Some(BuilderState::TableHeader) = self.state_stack.last() {
                self.state_stack.pop();
                header = Some(PreparsedTemplate(
                    self.instruction_stack.pop().unwrap_or_default(),
                ));
            }
            if let Some(BuilderState::TableColumns) = self.state_stack.last() {
                self.state_stack.pop();
                // Pop the (now empty) instruction vector associated with the <columns> element.
                self.instruction_stack.pop();
            }

            for dim_str in col_strings {
                columns.push(run_parser(style_parsers::parse_dimension, &dim_str)?);
            }

            let styles = self.resolve_styles(&attrs, location)?;
            let table_instr = XsltInstruction::Table {
                styles,
                columns,
                header,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(table_instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_table_columns_start(&mut self) {
        self.state_stack.push(BuilderState::TableColumns);
    }

    pub(crate) fn handle_table_header_start(&mut self) {
        self.state_stack.push(BuilderState::TableHeader);
    }

    pub(crate) fn handle_table_column(&mut self, attrs: OwnedAttributes) -> Result<(), XsltError> {
        if let Some(BuilderState::TableColumns) = self.state_stack.last() {
            let width = get_attr_owned_optional(&attrs, b"column-width")?
                .or(get_attr_owned_optional(&attrs, b"width")?);
            if let Some(w_str) = width {
                // Find the parent `Table` state on the stack and add the column width.
                // It will be the state before the current `TableColumns` state.
                let table_state_index = self.state_stack.len().saturating_sub(2);
                if let Some(BuilderState::Table { columns, .. }) =
                    self.state_stack.get_mut(table_state_index)
                {
                    columns.push(w_str);
                }
            }
        }
        Ok(())
    }
}
