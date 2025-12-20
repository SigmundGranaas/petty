//! Handlers for literal output, including text, `value-of`, and literal result elements.

use crate::ast::AttributeValueTemplate;
use crate::ast::{PreparsedStyles, PreparsedTemplate};
use crate::executor::{ExecutionError, TemplateExecutor};
use crate::output::OutputBuilder;
use petty_style::dimension::Dimension;
use petty_xpath1::datasource::DataSourceNode;
use petty_xpath1::{self, engine};
use std::collections::HashMap;

pub(crate) fn handle_text(text: &str, builder: &mut dyn OutputBuilder) {
    builder.add_text(text);
}

pub(crate) fn handle_value_of<'a, N: DataSourceNode<'a> + 'a>(
    select: &petty_xpath1::Expression,
    e_ctx: &engine::EvaluationContext<'a, '_, N>,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    let result = petty_xpath1::evaluate(select, e_ctx)?;
    let content = result.to_string();
    builder.add_text(&content);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_content_tag<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    tag_name: &[u8],
    styles: &PreparsedStyles,
    attrs: &HashMap<String, String>,
    body: &PreparsedTemplate,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    executor.execute_start_tag(tag_name, styles, builder);

    for (name, value) in attrs {
        builder.set_attribute(name, value);
    }

    executor.execute_template(body, context_node, context_position, context_size, builder)?;
    executor.execute_end_tag(tag_name, builder);
    Ok(())
}

pub(crate) fn handle_empty_tag<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    tag_name: &[u8],
    styles: &PreparsedStyles,
    attrs: &HashMap<String, String>,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    executor.execute_start_tag(tag_name, styles, builder);

    for (name, value) in attrs {
        builder.set_attribute(name, value);
    }

    executor.execute_end_tag(tag_name, builder);
    Ok(())
}

pub(crate) fn handle_element<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    name_avt: &AttributeValueTemplate,
    body: &PreparsedTemplate,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    let tag_name = {
        let merged_vars = executor.get_merged_variables();
        let e_ctx =
            executor.get_eval_context(context_node, &merged_vars, context_position, context_size);
        executor.evaluate_avt(name_avt, &e_ctx)?
    };

    // xsl:element does not have its own styling attributes. Styling must be applied
    // via xsl:attribute or by having the children handle it.
    let empty_styles = PreparsedStyles::default();

    executor.execute_start_tag(tag_name.as_bytes(), &empty_styles, builder);
    executor.execute_template(body, context_node, context_position, context_size, builder)?;
    executor.execute_end_tag(tag_name.as_bytes(), builder);
    Ok(())
}

pub(crate) fn handle_attribute<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    name_avt: &AttributeValueTemplate,
    body: &PreparsedTemplate,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    struct TextCollector(String);
    impl OutputBuilder for TextCollector {
        fn add_text(&mut self, text: &str) {
            self.0.push_str(text);
        }
        fn start_block(&mut self, _: &PreparsedStyles) {}
        fn end_block(&mut self) {}
        fn start_flex_container(&mut self, _: &PreparsedStyles) {}
        fn end_flex_container(&mut self) {}
        fn start_paragraph(&mut self, _: &PreparsedStyles) {}
        fn end_paragraph(&mut self) {}
        fn start_list(&mut self, _: &PreparsedStyles) {}
        fn end_list(&mut self) {}
        fn start_list_item(&mut self, _: &PreparsedStyles) {}
        fn end_list_item(&mut self) {}
        fn start_image(&mut self, _: &PreparsedStyles) {}
        fn end_image(&mut self) {}
        fn start_styled_span(&mut self, _: &PreparsedStyles) {}
        fn end_styled_span(&mut self) {}
        fn start_hyperlink(&mut self, _: &PreparsedStyles) {}
        fn end_hyperlink(&mut self) {}
        fn set_attribute(&mut self, _: &str, _: &str) {}
        fn start_table(&mut self, _: &PreparsedStyles) {}
        fn end_table(&mut self) {}
        fn set_table_columns(&mut self, _: &[Dimension]) {}
        fn start_table_header(&mut self) {}
        fn end_table_header(&mut self) {}
        fn start_table_row(&mut self, _: &PreparsedStyles) {}
        fn end_table_row(&mut self) {}
        fn start_table_cell(&mut self, _: &PreparsedStyles) {}
        fn end_table_cell(&mut self) {}
        fn start_heading(&mut self, _: &PreparsedStyles, _: u8) {}
        fn end_heading(&mut self) {}
        fn add_page_break(&mut self, _: Option<String>) {}
    }

    let name = {
        let merged_vars = executor.get_merged_variables();
        let e_ctx =
            executor.get_eval_context(context_node, &merged_vars, context_position, context_size);
        executor.evaluate_avt(name_avt, &e_ctx)?
    };

    let mut text_builder = TextCollector(String::new());
    executor.execute_template(
        body,
        context_node,
        context_position,
        context_size,
        &mut text_builder,
    )?;

    builder.set_attribute(&name, &text_builder.0);
    Ok(())
}
