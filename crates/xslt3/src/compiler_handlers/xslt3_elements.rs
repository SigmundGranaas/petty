//! XSLT 3.0-specific element handlers: xsl:map, xsl:array, merge components.

use crate::ast::{
    ArrayMemberInstruction, MapEntryInstruction, MergeAction, MergeKey, MergeSource,
    PreparsedTemplate, Xslt3Instruction,
};
use crate::compiler::{
    BuilderState3, CompilerBuilder3, OwnedAttributes, get_attr_optional, get_attr_required,
};
use crate::error::Xslt3Error;
use petty_xslt::ast::SortOrder;

impl CompilerBuilder3 {
    pub(crate) fn handle_map_entry_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let key_str = get_attr_required(&attrs, b"key", b"xsl:map-entry", pos, source)?;
        let key = self.parse_xpath(&key_str)?;

        self.state_stack.push(BuilderState3::MapEntry { key });
        Ok(())
    }

    pub(crate) fn handle_map_entry_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::MapEntry { key } = current_state {
            let (select, body_opt) = if body.is_empty() {
                (None, None)
            } else if body.len() == 1
                && let Some(Xslt3Instruction::Sequence { select }) = body.first()
            {
                (Some(select.clone()), None)
            } else {
                (None, Some(PreparsedTemplate(body)))
            };

            let entry = MapEntryInstruction {
                key,
                select,
                body: body_opt,
            };

            if let Some(BuilderState3::Map { entries }) = self.state_stack.last_mut() {
                entries.push(entry);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_map_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Map { entries } = current_state {
            let instr = Xslt3Instruction::Map { entries };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_map_entry_empty(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let key_str = get_attr_required(attrs, b"key", b"xsl:map-entry", pos, source)?;
        let key = self.parse_xpath(&key_str)?;
        let select = if let Some(select_str) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&select_str)?)
        } else {
            None
        };

        let entry = MapEntryInstruction {
            key,
            select,
            body: None,
        };

        if let Some(BuilderState3::Map { entries }) = self.state_stack.last_mut() {
            entries.push(entry);
        }
        Ok(())
    }

    pub(crate) fn handle_array_member_empty(
        &mut self,
        attrs: &OwnedAttributes,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        let select = if let Some(select_str) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&select_str)?)
        } else {
            None
        };

        let member = ArrayMemberInstruction { select, body: None };

        if let Some(BuilderState3::Array { members }) = self.state_stack.last_mut() {
            members.push(member);
        }
        Ok(())
    }

    pub(crate) fn handle_array_member_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        let (select, body_opt) = if body.is_empty() {
            (None, None)
        } else if body.len() == 1
            && let Some(Xslt3Instruction::Sequence { select }) = body.first()
        {
            (Some(select.clone()), None)
        } else {
            (None, Some(PreparsedTemplate(body)))
        };

        let member = ArrayMemberInstruction {
            select,
            body: body_opt,
        };

        if let Some(BuilderState3::Array { members }) = self.state_stack.last_mut() {
            members.push(member);
        }
        Ok(())
    }

    pub(crate) fn handle_array_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Array { members } = current_state {
            let instr = Xslt3Instruction::Array { members };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_fork_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Fork { branches } = current_state {
            let instr = Xslt3Instruction::Fork { branches };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_merge_start(&mut self) -> Result<(), Xslt3Error> {
        self.features.uses_merge = true;
        self.state_stack.push(BuilderState3::Merge {
            sources: Vec::new(),
            action: None,
        });
        Ok(())
    }

    pub(crate) fn handle_merge_source_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_optional(&attrs, b"name")?;
        let for_each_item = if let Some(s) = get_attr_optional(&attrs, b"for-each-item")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };
        let select_str = get_attr_required(&attrs, b"select", b"xsl:merge-source", pos, source)?;
        let select = self.parse_xpath(&select_str)?;
        let streamable = get_attr_optional(&attrs, b"streamable")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        self.state_stack.push(BuilderState3::MergeSource {
            name,
            for_each_item,
            select,
            sort_keys: Vec::new(),
            streamable,
        });
        Ok(())
    }

    pub(crate) fn handle_merge_key(
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
        let collation = get_attr_optional(attrs, b"collation")?;

        let key = MergeKey {
            select,
            order,
            collation,
        };

        if let Some(BuilderState3::MergeSource { sort_keys, .. }) = self.state_stack.last_mut() {
            sort_keys.push(key);
        }
        Ok(())
    }

    pub(crate) fn handle_merge_source_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::MergeSource {
            name,
            for_each_item,
            select,
            sort_keys,
            streamable,
        } = current_state
        {
            let source = MergeSource {
                name,
                for_each_item,
                for_each_source: None,
                select,
                sort_keys,
                streamable,
            };

            if let Some(BuilderState3::Merge { sources, .. }) = self.state_stack.last_mut() {
                sources.push(source);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_merge_action_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let Some(BuilderState3::Merge { action, .. }) = self.state_stack.last_mut() {
            *action = Some(MergeAction {
                body: PreparsedTemplate(body),
            });
        }
        Ok(())
    }

    pub(crate) fn handle_merge_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Merge { sources, action } = current_state {
            let action = action.unwrap_or(MergeAction {
                body: PreparsedTemplate(Vec::new()),
            });

            let instr = Xslt3Instruction::Merge { sources, action };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_result_document_start(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let format = get_attr_optional(attrs, b"format")?;
        let href = if let Some(h) = get_attr_optional(attrs, b"href")? {
            Some(self.parse_avt(&h)?)
        } else {
            None
        };

        self.state_stack
            .push(BuilderState3::ResultDocument { format, href });
        Ok(())
    }

    pub(crate) fn handle_result_document_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::ResultDocument { format, href } = current_state {
            let instr = Xslt3Instruction::ResultDocument {
                format,
                href,
                body: PreparsedTemplate(body),
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_json_to_xml(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let select_str = get_attr_required(attrs, b"select", b"xsl:json-to-xml", pos, source)?;
        let select = self.parse_xpath(&select_str)?;

        let instr = Xslt3Instruction::JsonToXml { select };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_xml_to_json(&mut self, attrs: &OwnedAttributes) -> Result<(), Xslt3Error> {
        let select = if let Some(s) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };

        let instr = Xslt3Instruction::XmlToJson { select };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_evaluate(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let xpath_str = get_attr_required(attrs, b"xpath", b"xsl:evaluate", pos, source)?;
        let xpath = self.parse_xpath(&xpath_str)?;

        let context_item = if let Some(s) = get_attr_optional(attrs, b"context-item")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };

        let namespace_context = if let Some(s) = get_attr_optional(attrs, b"namespace-context")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };

        let instr = Xslt3Instruction::Evaluate {
            xpath,
            context_item,
            namespace_context,
        };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_fallback_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        let instr = Xslt3Instruction::Fallback {
            body: PreparsedTemplate(body),
        };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}
