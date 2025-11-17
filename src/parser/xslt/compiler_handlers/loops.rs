use crate::parser::xslt::ast::{PreparsedTemplate, SortDataType, SortKey, SortOrder, XsltInstruction};
use crate::parser::xslt::compiler::{BuilderState, CompilerBuilder};
use crate::parser::ParseError;
use crate::parser::xslt::util::{get_attr_owned_optional, get_attr_owned_required, get_line_col_from_pos, OwnedAttributes};

impl CompilerBuilder {
    pub(crate) fn handle_sortable_start(&mut self, attrs: OwnedAttributes) {
        self.state_stack.push(BuilderState::Sortable {
            attrs,
            sort_keys: vec![],
            saw_non_sort_child: false,
        });
    }

    pub(crate) fn handle_sort(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let location = get_line_col_from_pos(source, pos).into();

        let select_str = get_attr_owned_optional(&attrs, b"select")?.unwrap_or_else(|| ".".to_string());
        let select_expr = self.parse_xpath_and_detect_features(&select_str)?;

        if let Some(BuilderState::Sortable { sort_keys, saw_non_sort_child, .. }) = self.state_stack.last_mut()
        {
            if *saw_non_sort_child {
                return Err(ParseError::TemplateStructure {
                    message: "<xsl:sort> must appear before any other content in its parent.".to_string(),
                    location,
                });
            }
            let order = match get_attr_owned_optional(&attrs, b"order")?.as_deref() {
                Some("descending") => SortOrder::Descending,
                _ => SortOrder::Ascending,
            };
            let data_type = match get_attr_owned_optional(&attrs, b"data-type")?.as_deref() {
                Some("number") => SortDataType::Number,
                _ => SortDataType::Text,
            };
            sort_keys.push(SortKey {
                select: select_expr,
                order,
                data_type,
            });
        } else {
            return Err(ParseError::TemplateStructure {
                message: "<xsl:sort> can only appear inside <xsl:for-each> or <xsl:apply-templates>.".to_string(),
                location,
            });
        }
        Ok(())
    }

    pub(crate) fn handle_for_each_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        if let BuilderState::Sortable { attrs, sort_keys, .. } = current_state {
            let select_str = get_attr_owned_required(
                &attrs,
                b"select",
                b"xsl:for-each",
                pos,
                source,
            )?;
            let instr = XsltInstruction::ForEach {
                select: self.parse_xpath_and_detect_features(&select_str)?,
                sort_keys,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_apply_templates_empty(&mut self, attrs: OwnedAttributes) -> Result<(), ParseError> {
        let select_expr = if let Some(select_str) = get_attr_owned_optional(&attrs, b"select")? {
            Some(self.parse_xpath_and_detect_features(&select_str)?)
        } else {
            None
        };
        let instr = XsltInstruction::ApplyTemplates {
            select: select_expr,
            mode: get_attr_owned_optional(&attrs, b"mode")?
                .map(|s| crate::parser::xslt::util::parse_avt(&s))
                .transpose()?,
            sort_keys: vec![],
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}