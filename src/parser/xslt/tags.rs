// src/parser/xslt/tags.rs
use super::nodes::parse_nodes;
use super::util::{
    get_attr_owned_optional, get_attr_owned_required, parse_table_columns, OwnedAttributes,
};
use super::XsltTemplateParser;
use crate::error::PipelineError;
use crate::idf::{FlexDirection, IDFEvent};
use crate::parser::processor::LayoutProcessorProxy;
use crate::xpath;
use quick_xml::events::Event as XmlEvent;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;
use std::borrow::Cow;
use std::io::BufRead;

/// A utility to capture the inner raw XML of a node.
/// Assumes the reader is positioned after the Start tag.
fn capture_inner_xml<B: BufRead>(
    reader: &mut Reader<B>,
    tag_name: QName,
) -> Result<String, PipelineError> {
    let mut buf = Vec::new();
    let mut writer_buf = Vec::new();
    let mut writer = quick_xml::Writer::new(&mut writer_buf);
    let mut depth = 0;

    loop {
        match reader.read_event_into(&mut buf)? {
            XmlEvent::Start(e) => {
                if e.name() == tag_name {
                    depth += 1;
                }
                writer.write_event(XmlEvent::Start(e))?;
            }
            XmlEvent::End(e) => {
                if e.name() == tag_name {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                writer.write_event(XmlEvent::End(e))?;
            }
            XmlEvent::Eof => {
                return Err(PipelineError::TemplateParseError(
                    "Unclosed tag while capturing inner XML".into(),
                ))
            }
            event => {
                writer.write_event(event)?;
            }
        }
        buf.clear();
    }
    drop(writer);
    Ok(String::from_utf8(writer_buf)?)
}

pub(super) async fn handle_xsl_for_each<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let tag_name = b"xsl:for-each";
    let path = get_attr_owned_required(attributes, b"select", tag_name)?;
    let selected_values = xpath::select(context, &path);

    let inner_xml = capture_inner_xml(reader, QName(tag_name))?;

    let items_to_iterate: Vec<&'a Value> = if let Some(first_val) = selected_values.get(0) {
        if let Some(arr) = first_val.as_array() { arr.iter().collect() } else { selected_values }
    } else { Vec::new() };

    if !inner_xml.is_empty() {
        for item_context in items_to_iterate {
            // Wrap fragment in a dummy root to ensure the parser terminates correctly.
            let wrapped_xml = format!("<petty-wrapper>{}</petty-wrapper>", inner_xml);
            let mut inner_reader = Reader::from_str(&wrapped_xml);
            inner_reader.config_mut().trim_text(false);
            let mut buf = Vec::new();

            // Consume the wrapper start tag before parsing its children.
            inner_reader.read_event_into(&mut buf)?;

            parse_nodes(parser, &mut inner_reader, item_context, proxy).await?;
        }
    }
    Ok(())
}

pub(super) async fn handle_xsl_if<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let tag_name = b"xsl:if";
    let test = get_attr_owned_required(attributes, b"test", tag_name)?;
    let results = xpath::select(context, &test);
    let condition_met = !results.is_empty() && results.iter().all(|v| !v.is_null());

    if condition_met {
        parse_nodes(parser, reader, context, proxy).await?;
    } else {
        // If condition is not met, we must consume the tag and its children to skip them.
        reader.read_to_end_into(QName(tag_name), &mut Vec::new())?;
    }
    Ok(())
}

pub(super) async fn handle_xsl_value_of<'a>(
    attributes: &OwnedAttributes,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let path = get_attr_owned_required(attributes, b"select", b"xsl:value-of")?;
    let content = xpath::select_as_string(context, &path);
    if !content.is_empty() {
        proxy
            .process_event(IDFEvent::AddText {
                content: Cow::Owned(content),
                style: None, // Inherit from parent
            })
            .await?;
    }
    Ok(())
}

pub(super) async fn handle_page_sequence<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let path = get_attr_owned_required(attributes, b"select", b"page-sequence")?;
    let selected_values = xpath::select(context, &path);
    let inner_xml = capture_inner_xml(reader, QName(b"page-sequence"))?;

    let items_to_iterate: Vec<&'a Value> = if path == "." || path == "/" {
        vec![context]
    } else if let Some(first_val) = selected_values.get(0) {
        if let Some(arr) = first_val.as_array() { arr.iter().collect() } else { selected_values }
    } else { Vec::new() };

    if !inner_xml.is_empty() {
        for item_context in items_to_iterate {
            proxy.process_event(IDFEvent::BeginPageSequence { context: item_context }).await?;

            // Wrap fragment in a dummy root to ensure the parser terminates correctly.
            let wrapped_xml = format!("<petty-wrapper>{}</petty-wrapper>", inner_xml);
            let mut inner_reader = Reader::from_str(&wrapped_xml);
            inner_reader.config_mut().trim_text(false);
            let mut buf = Vec::new();

            // Consume the wrapper start tag before parsing its children.
            inner_reader.read_event_into(&mut buf)?;

            parse_nodes(parser, &mut inner_reader, item_context, proxy).await?;
            proxy.process_event(IDFEvent::EndPageSequence).await?;
        }
    }
    Ok(())
}

pub(super) async fn handle_container<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    has_children: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    proxy.process_event(IDFEvent::StartBlock { style: style.map(Cow::Owned) }).await?;
    if has_children {
        parse_nodes(parser, reader, context, proxy).await?;
    }
    proxy.process_event(IDFEvent::EndBlock).await?;
    Ok(())
}

pub(super) async fn handle_flex_container<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    has_children: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    let direction_str = get_attr_owned_required(attributes, b"direction", b"flex-container")?;
    let direction = match direction_str.as_str() {
        "row" => crate::idf::FlexDirection::Row,
        "column" => crate::idf::FlexDirection::Column,
        _ => return Err(PipelineError::TemplateParseError(format!("Invalid direction for flex-container: {}", direction_str))),
    };

    proxy.process_event(IDFEvent::StartFlexContainer {
        style: style.map(Cow::Owned),
        direction,
    }).await?;

    if has_children {
        parse_nodes(parser, reader, context, proxy).await?;
    }

    proxy.process_event(IDFEvent::EndFlexContainer).await?;
    Ok(())
}

pub(super) async fn handle_text<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    has_children: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    // A <text> tag now creates a horizontal flexbox context for its mixed content.
    proxy
        .process_event(IDFEvent::StartFlexContainer {
            style: style.map(Cow::Owned),
            direction: FlexDirection::Row,
        })
        .await?;
    if has_children {
        parse_nodes(parser, reader, context, proxy).await?;
    }
    proxy.process_event(IDFEvent::EndFlexContainer).await?;
    Ok(())
}

pub(super) async fn handle_rectangle(attributes: &OwnedAttributes, proxy: &mut LayoutProcessorProxy<'_>) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    proxy.process_event(IDFEvent::AddRectangle { style: style.map(Cow::Owned) }).await
}

pub(super) async fn handle_image<'a>(parser: &mut XsltTemplateParser<'a>, attributes: &OwnedAttributes, context: &'a Value, proxy: &mut LayoutProcessorProxy<'a>) -> Result<(), PipelineError> {
    let src = get_attr_owned_required(attributes, b"src", b"image")?;
    let style = get_attr_owned_optional(attributes, b"style")?;
    let rendered_src = if src.contains("{{") {
        parser.template_engine.render_template(&src, context).map_err(|e| PipelineError::TemplateParseError(e.to_string()))?
    } else { src };
    proxy.process_event(IDFEvent::AddImage { src: Cow::Owned(rendered_src), style: style.map(Cow::Owned), data: None }).await
}

pub(super) async fn handle_link<'a>(parser: &mut XsltTemplateParser<'a>, attributes: &OwnedAttributes, has_children: bool, reader: &mut Reader<&[u8]>, context: &'a Value, proxy: &mut LayoutProcessorProxy<'a>) -> Result<(), PipelineError> {
    let href = get_attr_owned_required(attributes, b"href", b"link")?;
    let style = get_attr_owned_optional(attributes, b"style")?;
    let rendered_href = if href.contains("{{") {
        parser.template_engine.render_template(&href, context).map_err(|e| PipelineError::TemplateParseError(e.to_string()))?
    } else { href };

    proxy.process_event(IDFEvent::AddHyperlink { href: Cow::Owned(rendered_href), style: style.map(Cow::Owned) }).await?;
    if has_children { parse_nodes(parser, reader, context, proxy).await?; }
    proxy.process_event(IDFEvent::EndHyperlink).await?;
    Ok(())
}

pub(super) async fn handle_inline<'a>(parser: &mut XsltTemplateParser<'a>, style_name: &'static str, has_children: bool, reader: &mut Reader<&[u8]>, context: &'a Value, proxy: &mut LayoutProcessorProxy<'a>) -> Result<(), PipelineError> {
    proxy.process_event(IDFEvent::StartInline { style: Some(Cow::Borrowed(style_name)) }).await?;
    if has_children { parse_nodes(parser, reader, context, proxy).await?; }
    proxy.process_event(IDFEvent::EndInline).await?;
    Ok(())
}

pub(super) async fn handle_table<'a>(parser: &mut XsltTemplateParser<'a>, attributes: &OwnedAttributes, reader: &mut Reader<&[u8]>, context: &'a Value, proxy: &mut LayoutProcessorProxy<'a>) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    let inner_xml = capture_inner_xml(reader, QName(b"table"))?;

    let mut columns = Vec::new();
    let mut columns_reader = Reader::from_str(&inner_xml);
    columns_reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    // Pre-scan the inner XML just to find the <columns> definition.
    loop {
        match columns_reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) if e.name().as_ref() == b"columns" => {
                columns = parse_table_columns(&mut columns_reader, e.name())?;
                break; // Found it, we're done.
            }
            Ok(XmlEvent::Eof) => break, // Reached end without finding <columns>.
            Err(e) => return Err(e.into()),
            _ => (), // Ignore other tags during this scan.
        }
        buf.clear();
    }

    proxy.process_event(IDFEvent::StartTable { style: style.map(Cow::Owned), columns: Cow::Owned(columns) }).await?;

    // Now, parse the actual content of the table. We must wrap it for parse_nodes to work correctly.
    let wrapped_content = format!("<petty-wrapper>{}</petty-wrapper>", inner_xml);
    let mut content_reader = Reader::from_str(&wrapped_content);
    content_reader.config_mut().trim_text(false);
    let mut content_buf = Vec::new();

    // Consume the wrapper's start tag before passing the reader to parse_nodes
    content_reader.read_event_into(&mut content_buf)?;
    parse_nodes(parser, &mut content_reader, context, proxy).await?;

    proxy.process_event(IDFEvent::EndTable).await?;
    Ok(())
}

pub(super) async fn handle_row<'a>(parser: &mut XsltTemplateParser<'a>, has_children: bool, reader: &mut Reader<&[u8]>, context: &'a Value, proxy: &mut LayoutProcessorProxy<'a>) -> Result<(), PipelineError> {
    proxy.process_event(IDFEvent::StartRow { context }).await?;
    parser.row_column_index_stack.push(0);
    if has_children { parse_nodes(parser, reader, context, proxy).await?; }
    parser.row_column_index_stack.pop();
    proxy.process_event(IDFEvent::EndRow).await?;
    Ok(())
}

pub(super) async fn handle_cell<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    has_children: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let style_override = get_attr_owned_optional(attributes, b"style")?;
    let col_index = *parser.row_column_index_stack.last().ok_or_else(|| PipelineError::TemplateParseError("<cell> outside <row>".into()))?;

    proxy.process_event(IDFEvent::StartCell {
        column_index: col_index,
        style_override,
    }).await?;

    // Recursively parse the complex content inside the cell instead of treating it as plain text.
    // This allows tags like <container>, <text>, etc. to function correctly.
    if has_children {
        parse_nodes(parser, reader, context, proxy).await?;
    }

    proxy.process_event(IDFEvent::EndCell).await?;

    // Increment the column index for the next cell in the row.
    if let Some(idx) = parser.row_column_index_stack.last_mut() { *idx += 1; }
    Ok(())
}

pub(super) async fn handle_list<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    has_children: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    proxy.process_event(IDFEvent::StartList { style: style.map(Cow::Owned) }).await?;
    if has_children {
        parse_nodes(parser, reader, context, proxy).await?;
    }
    proxy.process_event(IDFEvent::EndList).await?;
    Ok(())
}

pub(super) async fn handle_list_item<'a>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    has_children: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;

    // A list item is a flex container with a bullet and the main content.
    proxy.process_event(IDFEvent::StartFlexContainer {
        style: style.map(Cow::Owned),
        direction: crate::idf::FlexDirection::Row,
    }).await?;

    // Add the bullet point as a simple text element. The trailing space/tab helps with alignment.
    proxy.process_event(IDFEvent::AddText {
        content: Cow::Borrowed("•\t"),
        style: Some(Cow::Borrowed("list-item-bullet")),
    }).await?;

    // The body of the list item is parsed directly into the flex container.
    // If it contains a <text> tag, it will be treated as the second flex item.
    if has_children {
        parse_nodes(parser, reader, context, proxy).await?;
    }

    proxy.process_event(IDFEvent::EndFlexContainer).await?; // End list item row
    Ok(())
}