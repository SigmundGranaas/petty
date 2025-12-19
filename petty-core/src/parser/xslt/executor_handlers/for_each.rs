// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/executor_handlers/for_each.rs
// FILE: src/parser/xslt/executor_handlers/for_each.rs
use crate::parser::xslt::datasource::DataSourceNode;
use crate::parser::xslt::xpath::{Expression, XPathValue};
use crate::parser::xslt::ast::{PreparsedTemplate, SortKey};
use crate::parser::xslt::executor::{ExecutionError, TemplateExecutor};
use crate::parser::xslt::output::OutputBuilder;

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

    if let XPathValue::NodeSet(mut nodes) = crate::parser::xslt::xpath::evaluate(select, &e_ctx)? {
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