//! Handlers for `<xsl:variable>`, `<xsl:param>`, and `<xsl:with-param>`.

use crate::ast::{Param, WithParam, XsltInstruction};
use crate::compiler::{BuilderState, CompilerBuilder};
use crate::util::{get_attr_owned_optional, get_attr_owned_required, get_line_col_from_pos, OwnedAttributes};
use crate::error::XsltError;

impl CompilerBuilder {
    pub(crate) fn handle_param(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), XsltError> {
        let location = get_line_col_from_pos(source, pos).into();

        let p_name = get_attr_owned_required(&attrs, b"name", b"xsl:param", pos, source)?;
        let select_expr = if let Some(s) = get_attr_owned_optional(&attrs, b"select")? {
            Some(self.parse_xpath_and_detect_features(&s)?)
        } else {
            None
        };

        if let Some(BuilderState::NamedTemplate { params, .. }) = self.state_stack.last_mut() {
            params.push(Param {
                name: p_name,
                default_value: select_expr,
            });
            Ok(())
        } else {
            Err(XsltError::TemplateStructure {
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
    ) -> Result<(), XsltError> {
        let location = get_line_col_from_pos(source, pos).into();

        let p_name = get_attr_owned_required(&attrs, b"name", b"xsl:with-param", pos, source)?;
        let select_str = get_attr_owned_required(&attrs, b"select", b"xsl:with-param", pos, source)?;
        let select_expr = self.parse_xpath_and_detect_features(&select_str)?;

        // `with-param` can be a child of `call-template` or `apply-templates` (which uses the Sortable state)
        if let Some(BuilderState::CallTemplate { params, .. }) = self.state_stack.last_mut() {
            let param = WithParam {
                name: p_name,
                select: select_expr,
            };
            params.push(param);
            Ok(())
        } else {
            // TODO: In the future, also check for BuilderState::Sortable for apply-templates
            Err(XsltError::TemplateStructure {
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
    ) -> Result<(), XsltError> {
        let select_str = get_attr_owned_required(
            &attrs,
            b"select",
            b"xsl:variable",
            pos,
            source,
        )?;
        let instr = XsltInstruction::Variable {
            name: get_attr_owned_required(&attrs, b"name", b"xsl:variable", pos, source)?,
            select: self.parse_xpath_and_detect_features(&select_str)?,
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}