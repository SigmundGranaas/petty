#![allow(clippy::too_many_arguments)]

use crate::ast::PreparsedTemplate;
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::Expression;
use petty_xpath31::types::XdmValue;
use petty_xslt::output::OutputBuilder;

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    pub(crate) fn handle_variable(
        &mut self,
        name: &str,
        select: &Option<Expression>,
        body: &Option<PreparsedTemplate>,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let value = if let Some(sel) = select {
            self.evaluate_xpath31_xdm(sel, context_node, context_position, context_size)?
        } else if let Some(body_template) = body {
            self.last_constructed_value = None;
            self.execute_template(
                body_template,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
            self.last_constructed_value
                .take()
                .unwrap_or_else(XdmValue::empty)
        } else {
            XdmValue::empty()
        };
        self.set_variable(name.to_string(), value);
        Ok(())
    }
}
