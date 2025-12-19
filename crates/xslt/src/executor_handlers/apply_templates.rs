use crate::ast::{AttributeValueTemplate, SortKey};
use crate::executor::{ExecutionError, TemplateExecutor};
use crate::output::OutputBuilder;
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath1::{Expression, XPathValue};

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_apply_templates<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    select: &Option<Expression>,
    mode_avt: &Option<AttributeValueTemplate>,
    sort_keys: &[SortKey],
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    let (mut nodes_to_process, merged_vars, evaluated_mode) = {
        let merged_vars = executor.get_merged_variables();
        let e_ctx =
            executor.get_eval_context(context_node, &merged_vars, context_position, context_size);

        let evaluated_mode = if let Some(avt) = mode_avt {
            Some(executor.evaluate_avt(avt, &e_ctx)?)
        } else {
            None
        };

        let nodes = if let Some(sel) = select {
            if let XPathValue::NodeSet(nodes) = petty_xpath1::evaluate(sel, &e_ctx)? {
                nodes
            } else {
                vec![]
            }
        } else {
            e_ctx.context_node.children().collect()
        };
        (nodes, merged_vars, evaluated_mode)
    };

    executor.sort_node_set(&mut nodes_to_process, sort_keys, &merged_vars)?;
    executor.apply_templates_to_nodes(&nodes_to_process, evaluated_mode.as_deref(), builder)?;
    Ok(())
}
