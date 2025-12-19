use petty_xpath1::datasource::DataSourceNode;
use petty_xpath1::{XPathValue};
use crate::executor::{ExecutionError, TemplateExecutor};

pub(crate) fn handle_variable<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    name: &str,
    value: XPathValue<N>,
) -> Result<(), ExecutionError> {
    executor.set_variable_in_current_scope(name.to_string(), value);
    Ok(())
}