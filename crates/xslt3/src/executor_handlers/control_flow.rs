//! Control flow execution: xsl:if, xsl:choose/when/otherwise, xsl:try/catch, xsl:next-match.

#![allow(clippy::too_many_arguments)]

use crate::ast::{CatchClause, PreparsedTemplate, When3};
use crate::executor::{ExecutionError, TemplateExecutor3};
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath31::Expression;
use petty_xpath31::types::XdmValue;
use petty_xslt::buffering_builder::BufferingOutputBuilder;
use petty_xslt::output::OutputBuilder;

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub code: String,
    pub description: String,
    pub value: Option<String>,
    pub module: Option<String>,
    pub line_number: Option<i64>,
    pub column_number: Option<i64>,
}

impl ErrorInfo {
    pub fn from_execution_error(e: &ExecutionError) -> Self {
        match e {
            ExecutionError::DynamicError { code, message } => Self {
                code: code.clone(),
                description: message.clone(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::XPath(msg) => Self {
                code: "XPTY0004".to_string(),
                description: msg.clone(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::TypeError(msg) => Self {
                code: "XPTY0004".to_string(),
                description: msg.clone(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::AssertionFailed(msg) => Self {
                code: "XTTE0000".to_string(),
                description: msg.clone(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::UnknownNamedTemplate(name) => Self {
                code: "XTDE0040".to_string(),
                description: format!("Unknown named template: {}", name),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::UnknownFunction(name) => Self {
                code: "XPST0017".to_string(),
                description: format!("Unknown function: {}", name),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::Break => Self {
                code: "XTDE0000".to_string(),
                description: "Unexpected break".to_string(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::NextIteration(_) => Self {
                code: "XTDE0000".to_string(),
                description: "Unexpected next-iteration".to_string(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::Stream(msg) => Self {
                code: "XTRE0000".to_string(),
                description: msg.clone(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::Resource(msg) => Self {
                code: "FODC0002".to_string(),
                description: msg.clone(),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
            ExecutionError::NoMatchingTemplate { node_name } => Self {
                code: "XTDE0555".to_string(),
                description: format!(
                    "No matching template for node '{}' (on-no-match=fail)",
                    node_name
                ),
                value: None,
                module: None,
                line_number: None,
                column_number: None,
            },
        }
    }
}

impl<'s, 'a, N: DataSourceNode<'a> + 'a> TemplateExecutor3<'s, 'a, N> {
    fn inject_error_variables(&mut self, error_info: &ErrorInfo) {
        self.set_variable(
            "err:code".to_string(),
            XdmValue::from_string(error_info.code.clone()),
        );
        self.set_variable(
            "err:description".to_string(),
            XdmValue::from_string(error_info.description.clone()),
        );

        let value = error_info.value.clone().unwrap_or_default();
        self.set_variable("err:value".to_string(), XdmValue::from_string(value));

        let module = error_info.module.clone().unwrap_or_default();
        self.set_variable("err:module".to_string(), XdmValue::from_string(module));

        let line = error_info.line_number.unwrap_or(0);
        self.set_variable("err:line-number".to_string(), XdmValue::from_integer(line));

        let col = error_info.column_number.unwrap_or(0);
        self.set_variable("err:column-number".to_string(), XdmValue::from_integer(col));
    }

    pub(crate) fn handle_try(
        &mut self,
        body: &PreparsedTemplate,
        catches: &[CatchClause],
        rollback_output: bool,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        if rollback_output {
            self.handle_try_with_rollback(
                body,
                catches,
                context_node,
                context_position,
                context_size,
                builder,
            )
        } else {
            self.handle_try_without_rollback(
                body,
                catches,
                context_node,
                context_position,
                context_size,
                builder,
            )
        }
    }

    fn handle_try_without_rollback(
        &mut self,
        body: &PreparsedTemplate,
        catches: &[CatchClause],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        match self.execute_template(body, context_node, context_position, context_size, builder) {
            Ok(()) => Ok(()),
            Err(e) => self.handle_catch(
                catches,
                &e,
                context_node,
                context_position,
                context_size,
                builder,
            ),
        }
    }

    fn handle_try_with_rollback(
        &mut self,
        body: &PreparsedTemplate,
        catches: &[CatchClause],
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let mut buffering = BufferingOutputBuilder::new(builder);
        buffering.start_buffering();

        match self.execute_template(
            body,
            context_node,
            context_position,
            context_size,
            &mut buffering,
        ) {
            Ok(()) => {
                buffering.flush();
                Ok(())
            }
            Err(e) => {
                buffering.discard();
                self.handle_catch(
                    catches,
                    &e,
                    context_node,
                    context_position,
                    context_size,
                    buffering.target_mut(),
                )
            }
        }
    }

    fn handle_catch(
        &mut self,
        catches: &[CatchClause],
        error: &ExecutionError,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let error_info = ErrorInfo::from_execution_error(error);
        let error_code = error_info.code.clone();

        for catch in catches {
            let matches = catch.errors.is_empty()
                || catch.errors.iter().any(|pattern| {
                    pattern == "*" || pattern == &error_code || error_code.starts_with(pattern)
                });

            if matches {
                self.push_scope();
                self.inject_error_variables(&error_info);

                let result = self.execute_template(
                    &catch.body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                );

                self.pop_scope();
                return result;
            }
        }

        Err(error.clone())
    }

    pub(crate) fn handle_if(
        &mut self,
        test: &Expression,
        body: &PreparsedTemplate,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let condition =
            self.evaluate_xpath31(test, context_node, context_position, context_size)?;
        if !condition.is_empty() && condition != "false" && condition != "0" {
            self.execute_template(body, context_node, context_position, context_size, builder)?;
        }
        Ok(())
    }

    pub(crate) fn handle_choose(
        &mut self,
        whens: &[When3],
        otherwise: &Option<PreparsedTemplate>,
        context_node: N,
        context_position: usize,
        context_size: usize,
        builder: &mut dyn OutputBuilder,
    ) -> Result<(), ExecutionError> {
        let mut matched = false;
        for when in whens {
            let condition =
                self.evaluate_xpath31(&when.test, context_node, context_position, context_size)?;
            if !condition.is_empty() && condition != "false" && condition != "0" {
                self.execute_template(
                    &when.body,
                    context_node,
                    context_position,
                    context_size,
                    builder,
                )?;
                matched = true;
                break;
            }
        }
        if !matched && let Some(otherwise_body) = otherwise {
            self.execute_template(
                otherwise_body,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_info_from_dynamic_error() {
        let error = ExecutionError::DynamicError {
            code: "XPTY0004".to_string(),
            message: "Type error".to_string(),
        };
        let info = ErrorInfo::from_execution_error(&error);
        assert_eq!(info.code, "XPTY0004");
        assert_eq!(info.description, "Type error");
    }

    #[test]
    fn test_error_info_from_xpath_error() {
        let error = ExecutionError::XPath("Invalid expression".to_string());
        let info = ErrorInfo::from_execution_error(&error);
        assert_eq!(info.code, "XPTY0004");
        assert_eq!(info.description, "Invalid expression");
    }

    #[test]
    fn test_error_info_from_type_error() {
        let error = ExecutionError::TypeError("Cannot convert".to_string());
        let info = ErrorInfo::from_execution_error(&error);
        assert_eq!(info.code, "XPTY0004");
        assert_eq!(info.description, "Cannot convert");
    }

    #[test]
    fn test_error_info_from_assertion_failed() {
        let error = ExecutionError::AssertionFailed("Check failed".to_string());
        let info = ErrorInfo::from_execution_error(&error);
        assert_eq!(info.code, "XTTE0000");
        assert_eq!(info.description, "Check failed");
    }

    #[test]
    fn test_error_info_from_unknown_template() {
        let error = ExecutionError::UnknownNamedTemplate("myTemplate".to_string());
        let info = ErrorInfo::from_execution_error(&error);
        assert_eq!(info.code, "XTDE0040");
        assert!(info.description.contains("myTemplate"));
    }

    #[test]
    fn test_error_info_from_unknown_function() {
        let error = ExecutionError::UnknownFunction("my:function".to_string());
        let info = ErrorInfo::from_execution_error(&error);
        assert_eq!(info.code, "XPST0017");
        assert!(info.description.contains("my:function"));
    }

    #[test]
    fn test_error_info_from_no_matching_template() {
        let error = ExecutionError::NoMatchingTemplate {
            node_name: "item".to_string(),
        };
        let info = ErrorInfo::from_execution_error(&error);
        assert_eq!(info.code, "XTDE0555");
        assert!(info.description.contains("item"));
    }
}
