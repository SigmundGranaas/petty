//! Streaming instruction handlers: xsl:stream, xsl:fork, xsl:merge, xsl:accumulator.

use crate::ast::{
    Accumulator, AccumulatorPhase, AccumulatorRule, Pattern3, PreparsedTemplate, Xslt3Instruction,
};
use crate::compiler::{
    BuilderState3, CompilerBuilder3, OwnedAttributes, get_attr_optional, get_attr_required,
};
use crate::error::Xslt3Error;
use crate::streaming::StreamabilityAnalyzer;

impl CompilerBuilder3 {
    pub(crate) fn handle_stream_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let href_str = get_attr_required(&attrs, b"href", b"xsl:stream", pos, source)?;
        let href = self.parse_avt(&href_str)?;

        self.state_stack.push(BuilderState3::SourceDocument {
            href,
            streamable: true,
        });
        Ok(())
    }

    pub(crate) fn handle_stream_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::SourceDocument {
            href,
            streamable: _,
        } = current_state
        {
            let body_template = PreparsedTemplate(body);

            StreamabilityAnalyzer::validate_streaming_template(&body_template)?;

            let instr = Xslt3Instruction::Stream {
                href,
                body: body_template,
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_source_document_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let href_str = get_attr_required(&attrs, b"href", b"xsl:source-document", pos, source)?;
        let href = self.parse_avt(&href_str)?;
        let streamable = get_attr_optional(&attrs, b"streamable")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        if streamable {
            self.features.uses_streaming = true;
        }

        self.state_stack
            .push(BuilderState3::SourceDocument { href, streamable });
        Ok(())
    }

    pub(crate) fn handle_source_document_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::SourceDocument { href, streamable } = current_state {
            let body_template = PreparsedTemplate(body);

            if streamable {
                StreamabilityAnalyzer::validate_streaming_template(&body_template)?;
            }

            let instr = Xslt3Instruction::SourceDocument {
                href,
                streamable,
                body: body_template,
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_stream_empty(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let href_str = get_attr_required(attrs, b"href", b"xsl:stream", pos, source)?;
        let href = self.parse_avt(&href_str)?;

        let instr = Xslt3Instruction::Stream {
            href,
            body: PreparsedTemplate(vec![]),
        };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_source_document_empty(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let href_str = get_attr_required(attrs, b"href", b"xsl:source-document", pos, source)?;
        let href = self.parse_avt(&href_str)?;
        let streamable = get_attr_optional(attrs, b"streamable")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        if streamable {
            self.features.uses_streaming = true;
        }

        let instr = Xslt3Instruction::SourceDocument {
            href,
            streamable,
            body: PreparsedTemplate(vec![]),
        };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_accumulator_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        self.features.uses_accumulators = true;
        let name = get_attr_required(&attrs, b"name", b"xsl:accumulator", pos, source)?;
        let initial_value = if let Some(s) = get_attr_optional(&attrs, b"initial-value")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };
        let streamable = get_attr_optional(&attrs, b"streamable")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        self.state_stack.push(BuilderState3::Accumulator {
            name,
            initial_value,
            rules: Vec::new(),
            streamable,
        });
        Ok(())
    }

    pub(crate) fn handle_accumulator_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Accumulator {
            name,
            initial_value,
            rules,
            streamable,
        } = current_state
        {
            let initial = initial_value.unwrap_or_else(|| {
                petty_xpath31::Expression::Literal(petty_xpath31::ast::Literal::String(
                    String::new(),
                ))
            });

            let accumulator = Accumulator {
                name: name.clone(),
                initial_value: initial,
                rules,
                streamable,
            };

            self.accumulators.insert(name, accumulator);
        }
        Ok(())
    }

    pub(crate) fn handle_accumulator_rule_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let match_str = get_attr_required(&attrs, b"match", b"xsl:accumulator-rule", pos, source)?;
        let phase = get_attr_optional(&attrs, b"phase")?
            .map(|s| {
                if s == "end" {
                    AccumulatorPhase::End
                } else {
                    AccumulatorPhase::Start
                }
            })
            .unwrap_or(AccumulatorPhase::Start);

        self.state_stack.push(BuilderState3::AccumulatorRule {
            pattern: Pattern3(match_str),
            phase,
        });
        Ok(())
    }

    pub(crate) fn handle_accumulator_rule_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::AccumulatorRule { pattern, phase } = current_state {
            let select = if body.len() == 1
                && let Some(Xslt3Instruction::Sequence { select }) = body.first()
            {
                select.clone()
            } else {
                petty_xpath31::Expression::Literal(petty_xpath31::ast::Literal::String(
                    String::new(),
                ))
            };

            let rule = AccumulatorRule {
                pattern,
                phase,
                select,
            };

            if let Some(BuilderState3::Accumulator { rules, .. }) = self.state_stack.last_mut() {
                rules.push(rule);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_accumulator_ref(
        &mut self,
        tag_name: &[u8],
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", tag_name, pos, source)?;

        let instr = if tag_name == b"xsl:accumulator-before" {
            Xslt3Instruction::AccumulatorBefore { name }
        } else {
            Xslt3Instruction::AccumulatorAfter { name }
        };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}
