use crate::ast::{PreparsedStyles, PreparsedTemplate};
use crate::executor::{ExecutionError, TemplateExecutor};
use crate::output::OutputBuilder;
use petty_xpath1::XPathValue;
use petty_xpath1::datasource::{DataSourceNode, NodeType};

pub(crate) fn handle_copy_of<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    result: XPathValue<N>,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    match result {
        XPathValue::NodeSet(nodes) => {
            for node in nodes {
                copy_data_source_node(executor, node, builder)?;
            }
        }
        _ => {
            let content = result.to_string();
            builder.add_text(&content);
        }
    }
    Ok(())
}

pub(crate) fn handle_copy<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    styles: &PreparsedStyles,
    body: &PreparsedTemplate,
    context_node: N,
    context_position: usize,
    context_size: usize,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    match context_node.node_type() {
        NodeType::Element => {
            let tag_name = context_node
                .name()
                .map_or(b"" as &[u8], |q| q.local_part.as_bytes());
            executor.execute_start_tag(tag_name, styles, builder);
            executor.execute_template(
                body,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
            executor.execute_end_tag(tag_name, builder);
        }
        NodeType::Text | NodeType::Attribute => {
            // Skip whitespace-only text nodes (like formatting between elements)
            let text = context_node.string_value();
            if !text.trim().is_empty() {
                builder.add_text(&text);
            }
        }
        NodeType::Root => {
            // Copying the root node just processes the children of the xsl:copy
            executor.execute_template(
                body,
                context_node,
                context_position,
                context_size,
                builder,
            )?;
        }
        NodeType::Comment | NodeType::ProcessingInstruction => {
            // The output format (IDF) does not support comments or PIs, so this is a no-op.
        }
    }
    Ok(())
}

/// Recursively transforms a `DataSourceNode` and its descendants into IDF nodes via the builder.
/// This is a semantic conversion, not a literal one.
fn copy_data_source_node<'s, 'a, N: DataSourceNode<'a> + 'a>(
    executor: &mut TemplateExecutor<'s, 'a, N>,
    node: N,
    builder: &mut dyn OutputBuilder,
) -> Result<(), ExecutionError> {
    match node.node_type() {
        NodeType::Element => {
            let local_name = node.name().map_or("", |q| q.local_part);
            let tag_name = local_name.as_bytes();

            // Simple semantic mapping
            executor.execute_start_tag(tag_name, &PreparsedStyles::default(), builder);

            // Copy attributes that have direct IDF mappings
            for attr in node.attributes() {
                if let Some(attr_name) = attr.name() {
                    builder.set_attribute(attr_name.local_part, &attr.string_value());
                }
            }

            // Recursively copy children
            for child in node.children() {
                copy_data_source_node(executor, child, builder)?;
            }

            executor.execute_end_tag(tag_name, builder);
        }
        NodeType::Text => {
            // Skip whitespace-only text nodes (like formatting between elements)
            let text = node.string_value();
            if !text.trim().is_empty() {
                builder.add_text(&text);
            }
        }
        // Root nodes are traversed, but don't create an output node themselves.
        NodeType::Root => {
            for child in node.children() {
                copy_data_source_node(executor, child, builder)?;
            }
        }
        // Attributes are handled when their parent element is copied.
        NodeType::Attribute => {}
        // The output format does not support these, so they are ignored.
        NodeType::Comment | NodeType::ProcessingInstruction => {}
    }
    Ok(())
}
