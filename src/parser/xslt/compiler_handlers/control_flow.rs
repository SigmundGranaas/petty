//! Handlers for XSLT control flow instructions like `<xsl:if>` and `<xsl:choose>`.

use crate::parser::xslt::ast::{PreparsedTemplate, When, XsltInstruction};
use crate::parser::xslt::compiler::{BuilderState, CompilerBuilder};
use crate::parser::ParseError;
use crate::parser::xslt::util::{get_attr_owned_required, get_line_col_from_pos, OwnedAttributes};

impl CompilerBuilder {
    pub(crate) fn handle_choose_start(&mut self) {
        self.state_stack
            .push(BuilderState::Choose { whens: vec![], otherwise: None });
    }

    pub(crate) fn handle_choose_end(&mut self, current_state: BuilderState) -> Result<(), ParseError> {
        if let BuilderState::Choose { whens, otherwise } = current_state {
            let instr = XsltInstruction::Choose { whens, otherwise };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_when_start(&mut self, attrs: OwnedAttributes, pos: usize, source: &str) -> Result<(), ParseError> {
        let location = get_line_col_from_pos(source, pos).into();
        if !matches!(self.state_stack.last(), Some(BuilderState::Choose { .. })) {
            return Err(ParseError::TemplateStructure {
                message: "<xsl:when> must be a direct child of <xsl:choose>".to_string(),
                location,
            });
        }
        self.state_stack.push(BuilderState::When(attrs));
        Ok(())
    }

    pub(crate) fn handle_when_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let location = get_line_col_from_pos(source, pos).into();
        if let BuilderState::When(attrs) = current_state {
            let test_str = get_attr_owned_required(
                &attrs,
                b"test",
                b"xsl:when",
                pos,
                source,
            )?;
            let test = self.parse_xpath_and_detect_features(&test_str)?;
            let when_block = When {
                test,
                body: PreparsedTemplate(body),
            };
            if let Some(BuilderState::Choose { whens, .. }) = self.state_stack.last_mut() {
                whens.push(when_block);
            } else {
                return Err(ParseError::TemplateStructure {
                    message: "Internal compiler error: <xsl:when> not inside <xsl:choose>.".to_string(),
                    location,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn handle_otherwise_start(&mut self, pos: usize, source: &str) -> Result<(), ParseError> {
        let location = get_line_col_from_pos(source, pos).into();
        if !matches!(self.state_stack.last(), Some(BuilderState::Choose { .. })) {
            return Err(ParseError::TemplateStructure {
                message: "<xsl:otherwise> must be a direct child of <xsl:choose>".to_string(),
                location,
            });
        }
        self.state_stack.push(BuilderState::Otherwise);
        Ok(())
    }

    pub(crate) fn handle_otherwise_end(
        &mut self,
        _current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let location = get_line_col_from_pos(source, pos).into();
        if let Some(BuilderState::Choose { otherwise, .. }) = self.state_stack.last_mut() {
            if otherwise.is_some() {
                return Err(ParseError::TemplateStructure {
                    message: "Only one <xsl:otherwise> is allowed inside <xsl:choose>".to_string(),
                    location,
                });
            }
            *otherwise = Some(PreparsedTemplate(body));
        } else {
            return Err(ParseError::TemplateStructure {
                message: "Internal compiler error: <xsl:otherwise> not inside <xsl:choose>.".to_string(),
                location,
            });
        }
        Ok(())
    }

    pub(crate) fn handle_if_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        if let BuilderState::InstructionBody(attrs) = current_state {
            let test_str = get_attr_owned_required(
                &attrs,
                b"test",
                b"xsl:if",
                pos,
                source,
            )?;
            let instr = XsltInstruction::If {
                test: self.parse_xpath_and_detect_features(&test_str)?,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }
}