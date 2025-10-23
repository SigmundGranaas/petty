use crate::parser::xslt::datasource::DataSourceNode;
use crate::parser::xslt::xpath::{XPathValue};
use crate::parser::xslt::executor::{ExecutionError, TemplateExecutor};

pub(crate) fn handle_variable<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    name: &str,
    value: XPathValue<N>,
) -> Result<(), ExecutionError> {
    executor.set_variable_in_current_scope(name.to_string(), value);
    Ok(())
}