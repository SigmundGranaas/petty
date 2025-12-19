use crate::ast::{PreparsedTemplate, When};
use crate::executor::{ExecutionError, TemplateExecutor};
use crate::output::OutputBuilder;
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath1::{self};

pub(crate) fn handle_if<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    condition: bool,
    body: &PreparsedTemplate,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    if condition {
        executor.execute_template(body, context_node, context_position, context_size, builder)?;
    }
    Ok(())
}

pub(crate) fn handle_choose<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    whens: &[When],
    otherwise: &Option<PreparsedTemplate>,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    let merged_vars = executor.get_merged_variables();
    let e_ctx =
        executor.get_eval_context(context_node, &merged_vars, context_position, context_size);

    let mut chose_one = false;
    for when_block in whens {
        if petty_xpath1::evaluate(&when_block.test, &e_ctx)?.to_bool() {
            executor.execute_template(
                &when_block.body,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
            chose_one = true;
            break;
        }
    }
    if !chose_one && let Some(otherwise_body) = otherwise {
        executor.execute_template(
            otherwise_body,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
    }
    Ok(())
}
