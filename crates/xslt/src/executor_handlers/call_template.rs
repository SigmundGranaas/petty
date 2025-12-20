use crate::ast::WithParam;
use crate::executor::{ExecutionError, TemplateExecutor};
use crate::output::OutputBuilder;
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath1::{self};
use std::collections::{HashMap, HashSet};

pub(crate) fn handle_call_template<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    name: &str,
    params: &[WithParam],
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    if let Some(template) = executor.stylesheet.named_templates.get(name) {
        let template_clone = template.clone();
        let caller_merged_vars = executor.get_merged_variables();

        let passed_params = {
            let e_ctx = executor.get_eval_context(
                context_node,
                &caller_merged_vars,
                context_position,
                context_size,
            );
            params
                .iter()
                .map(|param| {
                    Ok((
                        param.name.clone(),
                        petty_xpath1::evaluate(&param.select, &e_ctx)?,
                    ))
                })
                .collect::<Result<HashMap<_, _>, ExecutionError>>()?
        };

        // Strict mode check for undeclared parameters
        if executor.strict {
            let defined_param_names: HashSet<_> =
                template_clone.params.iter().map(|p| &p.name).collect();
            for passed_name in passed_params.keys() {
                if !defined_param_names.contains(passed_name) {
                    return Err(ExecutionError::TypeError(format!(
                        "Call to template '{}' with undeclared parameter: '{}'",
                        name, passed_name
                    )));
                }
            }
        }

        executor.push_scope();
        for defined_param in &template_clone.params {
            let param_value = if let Some(passed_value) = passed_params.get(&defined_param.name) {
                passed_value.clone()
            } else if let Some(default_expr) = &defined_param.default_value {
                let default_e_ctx = executor.get_eval_context(
                    context_node,
                    &caller_merged_vars,
                    context_position,
                    context_size,
                );
                petty_xpath1::evaluate(default_expr, &default_e_ctx)?
            } else {
                petty_xpath1::XPathValue::String("".to_string())
            };
            executor.set_variable_in_current_scope(defined_param.name.clone(), param_value);
        }
        executor.execute_template(
            &template_clone.body,
            context_node,
            context_position,
            context_size,
            builder,
        )?;
        executor.pop_scope();
        Ok(())
    } else {
        Err(ExecutionError::UnknownNamedTemplate(name.to_string()))
    }
}
