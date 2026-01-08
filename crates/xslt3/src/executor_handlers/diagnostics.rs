//! Diagnostic instruction execution: xsl:assert, xsl:message.

#![allow(clippy::too_many_arguments)]

use crate::ast::PreparsedTemplate;
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::Expression;
use petty_xslt::idf_builder::IdfBuilder;

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub(crate) fn handle_assert(
        &mut self,
        test: &Expression,
        message: &Option<PreparsedTemplate>,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<(), ExecutionError> {
        let condition =
            self.evaluate_xpath31(test, context_node, context_position, context_size)?;
        if condition.is_empty() || condition == "false" || condition == "0" {
            let msg = if let Some(msg_body) = message {
                let mut msg_builder = IdfBuilder::new();
                self.execute_template(
                    msg_body,
                    context_node,
                    context_position,
                    context_size,
                    &mut msg_builder,
                )?;
                "Assertion failed".to_string()
            } else {
                "Assertion failed".to_string()
            };
            return Err(ExecutionError::AssertionFailed(msg));
        }
        Ok(())
    }

    pub(crate) fn handle_message(
        &mut self,
        select: &Option<Expression>,
        body: &Option<PreparsedTemplate>,
        terminate: bool,
        error_code: &Option<String>,
        context_node: N,
        context_position: usize,
        context_size: usize,
    ) -> Result<(), ExecutionError> {
        let msg = if let Some(sel) = select {
            self.evaluate_xpath31(sel, context_node, context_position, context_size)?
        } else if let Some(body_template) = body {
            let mut msg_builder = IdfBuilder::new();
            self.execute_template(
                body_template,
                context_node,
                context_position,
                context_size,
                &mut msg_builder,
            )?;
            String::new()
        } else {
            String::new()
        };

        if terminate {
            let code = error_code.clone().unwrap_or_else(|| "XTMM9000".to_string());
            return Err(ExecutionError::DynamicError { code, message: msg });
        }
        Ok(())
    }
}
