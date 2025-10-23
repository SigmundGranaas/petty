// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/compiler_handlers/variables.rs
// FILE: src/parser/xslt/compiler_handlers/variables.rs
//! Handlers for `<xsl:variable>`, `<xsl:param>`, and `<xsl:with-param>`.

use crate::parser::xslt::ast::{Param, WithParam, XsltInstruction};
use crate::parser::xslt::compiler::{BuilderState, CompilerBuilder};
use crate::parser::xslt::util::{get_attr_owned_optional, get_attr_owned_required, get_line_col_from_pos, OwnedAttributes};
use crate::parser::ParseError;
use crate::parser::xslt::xpath;

impl CompilerBuilder {
    pub(crate) fn handle_param(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let location = get_line_col_from_pos(source, pos).into();
        if let Some(BuilderState::NamedTemplate { params, .. }) = self.state_stack.last_mut() {
            let p_name = get_attr_owned_required(&attrs, b"name", b"xsl:param", pos, source)?;
            let select = get_attr_owned_optional(&attrs, b"select")?
                .map(|s| xpath::parse_expression(&s))
                .transpose()?;
            params.push(Param {
                name: p_name,
                default_value: select,
            });
            Ok(())
        } else {
            Err(ParseError::TemplateStructure {
                message: "<xsl:param> can only appear at the top level of a named template.".to_string(),
                location,
            })
        }
    }

    pub(crate) fn handle_with_param(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let location = get_line_col_from_pos(source, pos).into();
        // `with-param` can be a child of `call-template` or `apply-templates` (which uses the Sortable state)
        if let Some(BuilderState::CallTemplate { params, .. }) = self.state_stack.last_mut() {
            let p_name = get_attr_owned_required(&attrs, b"name", b"xsl:with-param", pos, source)?;
            let select = get_attr_owned_required(&attrs, b"select", b"xsl:with-param", pos, source)?;
            let param = WithParam {
                name: p_name,
                select: xpath::parse_expression(&select)?,
            };
            params.push(param);
            Ok(())
        } else {
            // TODO: In the future, also check for BuilderState::Sortable for apply-templates
            Err(ParseError::TemplateStructure {
                message: "<xsl:with-param> must be a direct child of <xsl:call-template> or <xsl:apply-templates>.".to_string(),
                location,
            })
        }
    }

    pub(crate) fn handle_variable(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let instr = XsltInstruction::Variable {
            name: get_attr_owned_required(&attrs, b"name", b"xsl:variable", pos, source)?,
            select: xpath::parse_expression(&get_attr_owned_required(
                &attrs,
                b"select",
                b"xsl:variable",
                pos,
                source,
            )?)?,
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}