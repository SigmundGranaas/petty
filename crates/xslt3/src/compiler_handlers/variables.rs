//! Variable and parameter handlers: xsl:variable, xsl:param, xsl:with-param, xsl:number.

use crate::ast::{
    Avt3, GlobalParam, GlobalVariable, IterateParam, NextIterationParam, NumberLevel, Param3,
    PreparsedTemplate, WithParam3, Xslt3Instruction,
};
use crate::compiler::{
    BuilderState3, CompilerBuilder3, OwnedAttributes, get_attr_optional, get_attr_required,
};
use crate::error::Xslt3Error;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParamContext {
    NamedTemplate,
    Function,
    Iterate,
    Stylesheet,
    Other,
}

impl CompilerBuilder3 {
    fn get_param_context(&self) -> ParamContext {
        match self.state_stack.last() {
            Some(BuilderState3::NamedTemplate { .. }) => ParamContext::NamedTemplate,
            Some(BuilderState3::Function { .. }) => ParamContext::Function,
            Some(BuilderState3::Iterate { .. }) => ParamContext::Iterate,
            Some(BuilderState3::Stylesheet) => ParamContext::Stylesheet,
            _ => ParamContext::Other,
        }
    }

    pub(crate) fn handle_param(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:param", pos, source)?;
        let select_str = get_attr_optional(attrs, b"select")?;
        let as_type = get_attr_optional(attrs, b"as")?.and_then(|s| self.parse_sequence_type(&s));
        let required = get_attr_optional(attrs, b"required")?
            .map(|s| s == "yes")
            .unwrap_or(false);
        let tunnel = get_attr_optional(attrs, b"tunnel")?
            .map(|s| s == "yes")
            .unwrap_or(false);
        let static_param = get_attr_optional(attrs, b"static")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        let ctx = self.get_param_context();

        match ctx {
            ParamContext::NamedTemplate | ParamContext::Function => {
                let default_value = if let Some(s) = select_str {
                    Some(self.parse_xpath(&s)?)
                } else {
                    None
                };
                let param = Param3 {
                    name,
                    default_value,
                    as_type,
                    required,
                    tunnel,
                };
                match self.state_stack.last_mut() {
                    Some(BuilderState3::NamedTemplate { params, .. }) => params.push(param),
                    Some(BuilderState3::Function { params, .. }) => params.push(param),
                    _ => {}
                }
            }
            ParamContext::Iterate => {
                let select = if let Some(s) = select_str {
                    self.parse_xpath(&s)?
                } else {
                    self.parse_xpath("()")?
                };
                let param = IterateParam {
                    name,
                    select,
                    as_type,
                };
                if let Some(BuilderState3::Iterate { params, .. }) = self.state_stack.last_mut() {
                    params.push(param);
                }
            }
            ParamContext::Stylesheet => {
                let default_value = if let Some(s) = select_str {
                    Some(self.parse_xpath(&s)?)
                } else {
                    None
                };
                self.global_params.insert(
                    name.clone(),
                    GlobalParam {
                        name,
                        default_value,
                        as_type,
                        required,
                        static_param,
                    },
                );
            }
            ParamContext::Other => {}
        }
        Ok(())
    }

    pub(crate) fn handle_with_param(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:with-param", pos, source)?;
        let select_str = get_attr_optional(attrs, b"select")?.unwrap_or_else(|| "''".to_string());
        let select = self.parse_xpath(&select_str)?;
        let tunnel = get_attr_optional(attrs, b"tunnel")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        let with_param = WithParam3 {
            name,
            select,
            tunnel,
        };

        match self.state_stack.last_mut() {
            Some(BuilderState3::CallTemplate { params, .. }) => {
                params.push(with_param);
            }
            Some(BuilderState3::NextIteration { params, .. }) => {
                params.push(NextIterationParam {
                    name: with_param.name,
                    select: with_param.select,
                });
            }
            Some(BuilderState3::Sortable { .. }) => {
                if let Some(BuilderState3::CallTemplate { params, .. }) =
                    self.state_stack.iter_mut().rev().nth(1)
                {
                    params.push(with_param);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handles empty `<xsl:variable name="x" select="..."/>` elements.
    pub(crate) fn handle_variable(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:variable", pos, source)?;
        let select_str = get_attr_optional(attrs, b"select")?.unwrap_or_else(|| "''".to_string());
        let select = self.parse_xpath(&select_str)?;
        let as_type = get_attr_optional(attrs, b"as")?.and_then(|s| self.parse_sequence_type(&s));
        let visibility = get_attr_optional(attrs, b"visibility")?
            .map(|s| self.parse_visibility(&s))
            .unwrap_or_default();
        let static_var = get_attr_optional(attrs, b"static")?
            .map(|s| s == "yes")
            .unwrap_or(false);

        if matches!(self.state_stack.last(), Some(BuilderState3::Stylesheet)) {
            self.global_variables.insert(
                name.clone(),
                GlobalVariable {
                    name,
                    select,
                    as_type,
                    visibility,
                    static_var,
                },
            );
        } else {
            let instr = Xslt3Instruction::Variable {
                name,
                select: Some(select),
                body: None,
                as_type,
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    /// Handles start of `<xsl:variable name="x">...</xsl:variable>` elements with body content.
    pub(crate) fn handle_variable_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(&attrs, b"name", b"xsl:variable", pos, source)?;
        let select = if let Some(select_str) = get_attr_optional(&attrs, b"select")? {
            Some(self.parse_xpath(&select_str)?)
        } else {
            None
        };
        let as_type = get_attr_optional(&attrs, b"as")?.and_then(|s| self.parse_sequence_type(&s));

        self.state_stack.push(BuilderState3::Variable {
            name,
            select,
            as_type,
        });
        Ok(())
    }

    /// Handles end of `<xsl:variable name="x">...</xsl:variable>` elements.
    pub(crate) fn handle_variable_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Variable {
            name,
            select,
            as_type,
        } = current_state
        {
            // If select is provided, use it; otherwise use the body
            let (final_select, final_body) = if select.is_some() {
                (select, None)
            } else if !body.is_empty() {
                (None, Some(PreparsedTemplate(body)))
            } else {
                // Empty variable with no select - default to empty string
                (Some(self.parse_xpath("''")?), None)
            };

            let instr = Xslt3Instruction::Variable {
                name,
                select: final_select,
                body: final_body,
                as_type,
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_value_of(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let select_str = get_attr_required(attrs, b"select", b"xsl:value-of", pos, source)?;
        let select = self.parse_xpath(&select_str)?;
        let separator = if let Some(s) = get_attr_optional(attrs, b"separator")? {
            Some(self.parse_avt(&s)?)
        } else {
            None
        };

        let instr = Xslt3Instruction::ValueOf { select, separator };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_copy_of(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let select_str = get_attr_required(attrs, b"select", b"xsl:copy-of", pos, source)?;
        let select = self.parse_xpath(&select_str)?;

        let instr = Xslt3Instruction::CopyOf { select };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_sequence(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let select_str = get_attr_required(attrs, b"select", b"xsl:sequence", pos, source)?;
        let select = self.parse_xpath(&select_str)?;

        let instr = Xslt3Instruction::Sequence { select };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_number(&mut self, attrs: &OwnedAttributes) -> Result<(), Xslt3Error> {
        let level = match get_attr_optional(attrs, b"level")?.as_deref() {
            Some("single") | None => NumberLevel::Single,
            Some("multiple") => NumberLevel::Multiple,
            Some("any") => NumberLevel::Any,
            Some(other) => {
                return Err(Xslt3Error::compile(format!(
                    "Invalid level '{}' for xsl:number",
                    other
                )));
            }
        };

        let count = get_attr_optional(attrs, b"count")?;
        let from = get_attr_optional(attrs, b"from")?;

        let value = if let Some(v) = get_attr_optional(attrs, b"value")? {
            Some(self.parse_xpath(&v)?)
        } else {
            None
        };

        let format = if let Some(f) = get_attr_optional(attrs, b"format")? {
            self.parse_avt3(&f)?
        } else {
            Avt3::Static("1".to_string())
        };

        let lang = get_attr_optional(attrs, b"lang")?;
        let letter_value = get_attr_optional(attrs, b"letter-value")?;
        let grouping_separator = get_attr_optional(attrs, b"grouping-separator")?;
        let grouping_size =
            get_attr_optional(attrs, b"grouping-size")?.and_then(|s| s.parse::<u32>().ok());
        let ordinal = get_attr_optional(attrs, b"ordinal")?;

        let select = if let Some(s) = get_attr_optional(attrs, b"select")? {
            Some(self.parse_xpath(&s)?)
        } else {
            None
        };

        let instr = Xslt3Instruction::Number {
            level,
            count,
            from,
            value,
            format,
            lang,
            letter_value,
            grouping_separator,
            grouping_size,
            ordinal,
            select,
        };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}
