use petty_style::dimension::Dimension;
use crate::ast::{PreparsedStyles, PreparsedTemplate};
use petty_xpath1::datasource::DataSourceNode;
use crate::executor::{ExecutionError, TemplateExecutor};
use crate::output::OutputBuilder;

pub(crate) fn handle_table<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    styles: &PreparsedStyles,
    columns: &[Dimension],
    header: &Option<PreparsedTemplate>,
    body: &PreparsedTemplate,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    builder.start_table(styles);
    builder.set_table_columns(columns);

    if let Some(header_template) = header {
        builder.start_table_header();
        executor.execute_template(
            header_template,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
        builder.end_table_header();
    }

    executor.execute_template(
        body,
        context_node,
        context_position,
        context_size,
        builder,
    )?;

    builder.end_table();
    Ok(())
}