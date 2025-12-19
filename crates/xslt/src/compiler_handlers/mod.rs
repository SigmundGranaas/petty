pub(super) mod control_flow;
pub(super) mod loops;
pub(super) mod stylesheet;
pub(super) mod table;
pub(super) mod template;
pub(super) mod variables;

use std::sync::Arc;
use super::compiler::{BuilderState, CompilerBuilder};
use super::util::{get_attr_owned_required, get_line_col_from_pos, OwnedAttributes};
use petty_style::stylesheet::ElementStyle;
use crate::ast::{PreparsedTemplate, XsltInstruction};
use crate::error::XsltError;
use quick_xml::events::BytesEnd;

// These are handlers for common/shared logic that doesn't fit neatly into one file.
// They are implemented as methods on CompilerBuilder.

impl CompilerBuilder {
    pub(crate) fn handle_literal_result_element_start(&mut self, attrs: OwnedAttributes) {
        self.state_stack.push(BuilderState::InstructionBody(attrs));
    }

    pub(crate) fn handle_literal_result_element_end(
        &mut self,
        e: &BytesEnd,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        let location = get_line_col_from_pos(source, pos).into();
        if let BuilderState::InstructionBody(attrs) = current_state {
            let styles = self.resolve_styles(&attrs, location)?;
            let non_style_attrs = super::util::get_non_style_attributes(self, &attrs)?;
            let instr = XsltInstruction::ContentTag {
                tag_name: e.name().as_ref().to_vec(),
                styles,
                attrs: non_style_attrs,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_element_start(&mut self, attrs: OwnedAttributes) {
        self.state_stack.push(BuilderState::InstructionElement(attrs));
    }

    pub(crate) fn handle_element_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        if let BuilderState::InstructionElement(attrs) = current_state {
            let name_avt_str = get_attr_owned_required(&attrs, b"name", b"xsl:element", pos, source)?;
            let instr = XsltInstruction::Element {
                name: crate::util::parse_avt(self, &name_avt_str)?,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_copy_start(&mut self, attrs: OwnedAttributes) {
        self.state_stack
            .push(BuilderState::InstructionBody(attrs));
    }

    pub(crate) fn handle_copy_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        let location = crate::util::get_line_col_from_pos(source, pos).into();
        if let BuilderState::InstructionBody(attrs) = current_state {
            let instr = XsltInstruction::Copy {
                styles: self.resolve_styles(&attrs, location)?,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_attribute_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        match self.state_stack.last() {
            Some(BuilderState::AttributeSet { .. }) => {
                let attr_name = get_attr_owned_required(&attrs, b"name", b"xsl:attribute", pos, source)?;
                self.state_stack.push(BuilderState::Attribute(attr_name));
            }
            _ => {
                self.state_stack
                    .push(BuilderState::InstructionAttribute(attrs));
            }
        }
        Ok(())
    }

    pub(crate) fn handle_attribute_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        match current_state {
            BuilderState::Attribute(prop) => {
                let value = body
                    .iter()
                    .find_map(|i| {
                        if let XsltInstruction::Text(t) = i {
                            Some(t.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if let Some(BuilderState::AttributeSet { style, .. }) = self.state_stack.last_mut() {
                    petty_style::parsers::apply_style_property(style, &prop, &value)?;
                }
            }
            BuilderState::InstructionAttribute(attrs) => {
                let attr_name_str = get_attr_owned_required(&attrs, b"name", b"xsl:attribute", pos, source)?;
                let instr = XsltInstruction::Attribute {
                    name: crate::util::parse_avt(self, &attr_name_str)?,
                    body: PreparsedTemplate(body),
                };
                if let Some(parent) = self.instruction_stack.last_mut() {
                    parent.push(instr);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_attribute_set_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        let name = get_attr_owned_required(&attrs, b"name", b"xsl:attribute-set", pos, source)?;
        self.state_stack.push(BuilderState::AttributeSet {
            name,
            style: ElementStyle::default(),
        });
        Ok(())
    }

    pub(crate) fn handle_attribute_set_end(&mut self, current_state: BuilderState) -> Result<(), XsltError> {
        if let BuilderState::AttributeSet { name, style } = current_state {
            self.stylesheet.styles.insert(name, Arc::new(style));
        }
        Ok(())
    }
}