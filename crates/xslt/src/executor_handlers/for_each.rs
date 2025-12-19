use petty_xpath1::datasource::DataSourceNode;
use petty_xpath1::{Expression, XPathValue};
use crate::ast::{PreparsedTemplate, SortKey};
use crate::executor::{ExecutionError, TemplateExecutor};
use crate::output::OutputBuilder;

pub(crate) fn handle_for_each<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    select: &Expression,
    sort_keys: &[SortKey],
    body: &PreparsedTemplate,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    let merged_vars = executor.get_merged_variables();
    let e_ctx = executor.get_eval_context(context_node, &merged_vars, context_position, context_size);

    if let XPathValue::NodeSet(mut nodes) = petty_xpath1::evaluate(select, &e_ctx)? {
        executor.sort_node_set(&mut nodes, sort_keys, &merged_vars)?;
        let inner_context_size = nodes.len();
        for (i, node) in nodes.into_iter().enumerate() {
            let inner_context_position = i + 1;
            executor.push_scope();
            executor.execute_template(
                body,
                node,
                inner_context_position,
                inner_context_size,
                builder,
            )?;
            executor.pop_scope();
        }
    }
    Ok(())
}