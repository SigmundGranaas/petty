//! Control flow instruction handlers: xsl:if, xsl:choose/when/otherwise, xsl:try/catch,
//! xsl:assert, xsl:mode, and xsl:analyze-string.

use crate::ast::{
    CatchClause, ModeDeclaration, OnMultipleMatch, OnNoMatch, PreparsedTemplate, TypedMode, When3,
    Xslt3Instruction,
};
use crate::compiler::{
    BuilderState3, CompilerBuilder3, OwnedAttributes, get_attr_optional, get_attr_required,
};
use crate::error::Xslt3Error;

impl CompilerBuilder3 {
    pub(crate) fn handle_if_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::InstructionBody(attrs) = current_state {
            let test_str = get_attr_required(&attrs, b"test", b"xsl:if", pos, source)?;
            let test = self.parse_xpath(&test_str)?;

            let instr = Xslt3Instruction::If {
                test,
                body: PreparsedTemplate(body),
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_when_start(
        &mut self,
        attrs: OwnedAttributes,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        self.state_stack.push(BuilderState3::When(attrs));
        Ok(())
    }

    pub(crate) fn handle_when_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::When(attrs) = current_state {
            let test_str = get_attr_required(&attrs, b"test", b"xsl:when", pos, source)?;
            let test = self.parse_xpath(&test_str)?;

            let when = When3 {
                test,
                body: PreparsedTemplate(body),
            };

            if let Some(BuilderState3::Choose { whens, .. }) = self.state_stack.last_mut() {
                whens.push(when);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_otherwise_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let Some(BuilderState3::Choose { otherwise, .. }) = self.state_stack.last_mut() {
            *otherwise = Some(PreparsedTemplate(body));
        }
        Ok(())
    }

    pub(crate) fn handle_choose_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Choose { whens, otherwise } = current_state {
            let instr = Xslt3Instruction::Choose { whens, otherwise };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_try_start(&mut self, attrs: &OwnedAttributes) -> Result<(), Xslt3Error> {
        self.features.uses_try_catch = true;
        let rollback_output = get_attr_optional(attrs, b"rollback-output")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        self.state_stack.push(BuilderState3::Try {
            body_instructions: Vec::new(),
            catches: Vec::new(),
            rollback_output,
        });
        Ok(())
    }

    pub(crate) fn handle_catch_start(&mut self, attrs: &OwnedAttributes) -> Result<(), Xslt3Error> {
        let errors_str = get_attr_optional(attrs, b"errors")?.unwrap_or_else(|| "*".to_string());
        let errors: Vec<String> = errors_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        self.state_stack.push(BuilderState3::Catch { errors });
        Ok(())
    }

    pub(crate) fn handle_try_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Try {
            mut body_instructions,
            catches,
            rollback_output,
        } = current_state
        {
            body_instructions.extend(body);
            let instr = Xslt3Instruction::Try {
                body: PreparsedTemplate(body_instructions),
                catches,
                rollback_output,
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_catch_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Catch { errors } = current_state {
            let catch = CatchClause {
                errors,
                body: PreparsedTemplate(body),
            };

            if let Some(BuilderState3::Try { catches, .. }) = self.state_stack.last_mut() {
                catches.push(catch);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_message_start(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let select = if let Some(s) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };
        let terminate = get_attr_optional(attrs, b"terminate")?
            .map(|s| s == "yes")
            .unwrap_or(false);
        let error_code = get_attr_optional(attrs, b"error-code")?;

        self.state_stack.push(BuilderState3::Message {
            select,
            terminate,
            error_code,
        });
        Ok(())
    }

    pub(crate) fn handle_message_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Message {
            select,
            terminate,
            error_code,
        } = current_state
        {
            let body_opt = if body.is_empty() {
                None
            } else {
                Some(PreparsedTemplate(body))
            };

            let instr = Xslt3Instruction::Message {
                select,
                body: body_opt,
                terminate,
                error_code,
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_assert_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        self.features.uses_assertions = true;
        let test_str = get_attr_required(&attrs, b"test", b"xsl:assert", pos, source)?;
        let test = self.parse_xpath(&test_str)?;

        self.state_stack.push(BuilderState3::Assert { test });
        Ok(())
    }

    pub(crate) fn handle_assert_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Assert { test } = current_state {
            let message = if body.is_empty() {
                None
            } else {
                Some(PreparsedTemplate(body))
            };

            let instr = Xslt3Instruction::Assert { test, message };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_next_match(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let params = Vec::new();

        let _ = (attrs, pos, source);

        let instr = Xslt3Instruction::NextMatch { params };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_apply_imports(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let params = Vec::new();
        let _ = attrs;

        let instr = Xslt3Instruction::ApplyImports { params };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_mode(&mut self, attrs: &OwnedAttributes) -> Result<(), Xslt3Error> {
        let name = get_attr_optional(attrs, b"name")?;
        let streamable = get_attr_optional(attrs, b"streamable")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        let on_no_match = get_attr_optional(attrs, b"on-no-match")?
            .map(|s| match s.as_str() {
                "shallow-skip" => OnNoMatch::ShallowSkip,
                "deep-copy" => OnNoMatch::DeepCopy,
                "shallow-copy" => OnNoMatch::ShallowCopy,
                "text-only-copy" => OnNoMatch::TextOnlyCopy,
                "fail" => OnNoMatch::Fail,
                _ => OnNoMatch::DeepSkip,
            })
            .unwrap_or_default();

        let on_multiple_match = get_attr_optional(attrs, b"on-multiple-match")?
            .map(|s| match s.as_str() {
                "fail" => OnMultipleMatch::Fail,
                _ => OnMultipleMatch::UseLast,
            })
            .unwrap_or_default();

        let warning_on_no_match = get_attr_optional(attrs, b"warning-on-no-match")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        let warning_on_multiple_match = get_attr_optional(attrs, b"warning-on-multiple-match")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        let visibility = get_attr_optional(attrs, b"visibility")?
            .map(|s| self.parse_visibility(&s))
            .unwrap_or_default();

        let typed = get_attr_optional(attrs, b"typed")?
            .map(|s| match s.as_str() {
                "strict" => TypedMode::Strict,
                "lax" => TypedMode::Lax,
                "untyped" => TypedMode::Untyped,
                _ => TypedMode::Unspecified,
            })
            .unwrap_or_default();

        let mode_decl = ModeDeclaration {
            name: name.clone(),
            streamable,
            on_no_match,
            on_multiple_match,
            warning_on_no_match,
            warning_on_multiple_match,
            visibility,
            typed,
        };

        self.modes.insert(name, mode_decl);
        Ok(())
    }

    pub(crate) fn handle_analyze_string_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let select_str = get_attr_required(&attrs, b"select", b"xsl:analyze-string", pos, source)?;
        let select = self.parse_xpath(&select_str)?;

        let regex = get_attr_required(&attrs, b"regex", b"xsl:analyze-string", pos, source)?;
        let flags = get_attr_optional(&attrs, b"flags")?;

        self.state_stack.push(BuilderState3::AnalyzeString {
            select,
            regex,
            flags,
            matching_substring: None,
            non_matching_substring: None,
        });
        Ok(())
    }

    pub(crate) fn handle_matching_substring_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let Some(BuilderState3::AnalyzeString {
            matching_substring, ..
        }) = self.state_stack.last_mut()
        {
            *matching_substring = Some(PreparsedTemplate(body));
        }
        Ok(())
    }

    pub(crate) fn handle_non_matching_substring_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let Some(BuilderState3::AnalyzeString {
            non_matching_substring,
            ..
        }) = self.state_stack.last_mut()
        {
            *non_matching_substring = Some(PreparsedTemplate(body));
        }
        Ok(())
    }

    pub(crate) fn handle_analyze_string_end(
        &mut self,
        current_state: BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::AnalyzeString {
            select,
            regex,
            flags,
            matching_substring,
            non_matching_substring,
        } = current_state
        {
            let instr = Xslt3Instruction::AnalyzeString {
                select,
                regex,
                flags,
                matching_substring,
                non_matching_substring,
            };

            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_perform_sort_start(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let select = if let Some(s) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };

        self.state_stack.push(BuilderState3::PerformSort {
            select,
            sort_keys: Vec::new(),
        });
        Ok(())
    }

    pub(crate) fn handle_perform_sort_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::PerformSort { select, sort_keys } = current_state {
            let instr = Xslt3Instruction::PerformSort {
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
}
