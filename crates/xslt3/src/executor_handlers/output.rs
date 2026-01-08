//! Text output execution: xsl:text, xsl:value-of, text value templates.

#![allow(clippy::too_many_arguments)]

use crate::ast::{PreparsedTemplate, TextValueTemplate};
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::Expression;
use petty_xslt::ast::AttributeValueTemplate;
use petty_xslt::output::OutputBuilder;

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub(crate) fn handle_text(&mut self, text: &str, builder: &mut dyn OutputBuilder) {
        self.add_text_with_character_maps(text, builder);
    }

    pub(crate) fn handle_text_value_template(
        &mut self,
        tvt: &TextValueTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let text = self.evaluate_tvt(tvt, context_node, context_position, context_size)?;
        self.add_text_with_character_maps(&text, builder);
        Ok(())
    }

    pub(crate) fn handle_value_of(
        &mut self,
        select: &Expression,
        separator: &Option<AttributeValueTemplate>,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let value = self.evaluate_xpath31(select, context_node, context_position, context_size)?;
        let _sep = if let Some(avt) = separator {
            self.evaluate_avt(avt, context_node, context_position, context_size)?
        } else {
            " ".to_string()
        };
        self.add_text_with_character_maps(&value, builder);
        Ok(())
    }

    pub(crate) fn handle_sequence(
        &mut self,
        select: &Expression,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<(), ExecutionError> {
        let _value = self.evaluate_xpath31(select, context_node, context_position, context_size)?;
        Ok(())
    }

    pub(crate) fn handle_copy_of(
        &mut self,
        select: &Expression,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let value = self.evaluate_xpath31(select, context_node, context_position, context_size)?;
        self.add_text_with_character_maps(&value, builder);
        Ok(())
    }

    pub(crate) fn handle_comment(
        &mut self,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        self.execute_template(body, context_node, context_position, context_size, builder)
    }

    pub(crate) fn handle_processing_instruction(
        &mut self,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        self.execute_template(body, context_node, context_position, context_size, builder)
    }
}
