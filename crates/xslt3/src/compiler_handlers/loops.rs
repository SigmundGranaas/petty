use crate::ast::{PreparsedTemplate, SortKey3, Xslt3Instruction};
use crate::compiler::{
    BuilderState3, CompilerBuilder3, OwnedAttributes, get_attr_optional, get_attr_required,
};
use crate::error::Xslt3Error;
use petty_xslt::ast::{SortDataType, SortOrder};

impl CompilerBuilder3 {
    pub(crate) fn handle_for_each_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Sortable {
            attrs, sort_keys, ..
        } = current_state
        {
            let select_str = get_attr_required(&attrs, b"select", b"xsl:for-each", pos, source)?;
            let select = self.parse_xpath(&select_str)?;

            let instr = Xslt3Instruction::ForEach {
                select,
                sort_keys,
                body: PreparsedTemplate(body),
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_for_each_group_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Sortable {
            attrs, sort_keys, ..
        } = current_state
        {
            let select_str =
                get_attr_required(&attrs, b"select", b"xsl:for-each-group", pos, source)?;
            let select = self.parse_xpath(&select_str)?;

            let group_by = if let Some(s) = get_attr_optional(&attrs, b"group-by")? {
                Some(self.parse_xpath(&s)?)
            } else {
                None
            };

            let group_adjacent = if let Some(s) = get_attr_optional(&attrs, b"group-adjacent")? {
                Some(self.parse_xpath(&s)?)
            } else {
                None
            };

            let group_starting_with = get_attr_optional(&attrs, b"group-starting-with")?;
            let group_ending_with = get_attr_optional(&attrs, b"group-ending-with")?;

            let instr = Xslt3Instruction::ForEachGroup {
                select,
                group_by,
                group_adjacent,
                group_starting_with,
                group_ending_with,
                sort_keys,
                body: PreparsedTemplate(body),
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_apply_templates_empty(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let select = if let Some(s) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };
        let mode = if let Some(m) = get_attr_optional(attrs, b"mode")? {
            Some(self.parse_avt(&m)?)
        } else {
            None
        };

        let instr = Xslt3Instruction::ApplyTemplates {
            select,
            mode,
            sort_keys: Vec::new(),
        };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_apply_templates_end(
        &mut self,
        current_state: BuilderState3,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Sortable {
            attrs, sort_keys, ..
        } = current_state
        {
            let select = if let Some(s) = get_attr_optional(&attrs, b"select")? {
                Some(self.parse_xpath(&s)?)
            } else {
                None
            };
            let mode = if let Some(m) = get_attr_optional(&attrs, b"mode")? {
                Some(self.parse_avt(&m)?)
            } else {
                None
            };

            let instr = Xslt3Instruction::ApplyTemplates {
                select,
                mode,
                sort_keys,
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_sort(
        &mut self,
        attrs: &OwnedAttributes,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        let select_str = get_attr_optional(attrs, b"select")?.unwrap_or_else(|| ".".to_string());
        let select = self.parse_xpath(&select_str)?;

        let order = get_attr_optional(attrs, b"order")?
            .map(|s| {
                if s == "descending" {
                    SortOrder::Descending
                } else {
                    SortOrder::Ascending
                }
            })
            .unwrap_or(SortOrder::Ascending);

        let data_type = get_attr_optional(attrs, b"data-type")?
            .map(|s| {
                if s == "number" {
                    SortDataType::Number
                } else {
                    SortDataType::Text
                }
            })
            .unwrap_or(SortDataType::Text);

        let collation = get_attr_optional(attrs, b"collation")?;
        let stable = get_attr_optional(attrs, b"stable")?.map(|s| s == "yes");

        let sort_key = SortKey3 {
            select,
            order,
            data_type,
            collation,
            stable,
        };

        if let Some(BuilderState3::Sortable { sort_keys, .. }) = self.state_stack.last_mut() {
            sort_keys.push(sort_key);
        }
        Ok(())
    }

    pub(crate) fn handle_call_template_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(&attrs, b"name", b"xsl:call-template", pos, source)?;
        self.state_stack.push(BuilderState3::CallTemplate {
            name,
            params: Vec::new(),
        });
        Ok(())
    }

    pub(crate) fn handle_call_template_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::CallTemplate { name, params } = current_state {
            let instr = Xslt3Instruction::CallTemplate { name, params };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_iterate_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        self.features.uses_iterate = true;
        let select_str = get_attr_required(&attrs, b"select", b"xsl:iterate", pos, source)?;
        let select = self.parse_xpath(&select_str)?;

        self.state_stack.push(BuilderState3::Iterate {
            select,
            params: Vec::new(),
            body_instructions: Vec::new(),
            on_completion: None,
        });
        Ok(())
    }

    pub(crate) fn handle_iterate_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Iterate {
            select,
            params,
            mut body_instructions,
            on_completion,
        } = current_state
        {
            body_instructions.extend(body);
            let instr = Xslt3Instruction::Iterate {
                select,
                params,
                body: PreparsedTemplate(body_instructions),
                on_completion,
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_on_completion_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let Some(BuilderState3::Iterate { on_completion, .. }) = self.state_stack.last_mut() {
            *on_completion = Some(PreparsedTemplate(body));
        }
        Ok(())
    }

    pub(crate) fn handle_next_iteration(
        &mut self,
        _attrs: &OwnedAttributes,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        let instr = Xslt3Instruction::NextIteration { params: Vec::new() };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_next_iteration_start(&mut self) -> Result<(), Xslt3Error> {
        self.state_stack
            .push(BuilderState3::NextIteration { params: Vec::new() });
        Ok(())
    }

    pub(crate) fn handle_next_iteration_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::NextIteration { params } = current_state {
            let instr = Xslt3Instruction::NextIteration { params };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_break(
        &mut self,
        attrs: &OwnedAttributes,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        let select = if let Some(s) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };

        let instr = Xslt3Instruction::Break { select };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}
