// src/parser/xslt/tags.rs
use super::nodes::parse_nodes;
use super::util::{
    capture_events_until_end, get_attr_owned_optional, get_attr_owned_required,
    parse_table_columns, parse_text_content, OwnedAttributes,
};
use super::XsltTemplateParser;
use crate::error::PipelineError;
use crate::layout::StreamingLayoutProcessor;
use crate::parser::Event;
use crate::render::DocumentRenderer;
use crate::xpath;
use log::debug;
use quick_xml::events::Event as XmlEvent;
use quick_xml::name::QName;
use quick_xml::Reader;
use quick_xml::Writer;
use serde_json::Value;
use std::borrow::Cow;

// --- Control Flow & Data Tag Handlers ---

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_xsl_for_each<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let tag_name = b"xsl:for-each";
    let path = get_attr_owned_required(attributes, b"select", tag_name)?;
    let selected_values = xpath::select(context, &path);

    let inner_events = if !is_empty {
        capture_events_until_end(reader, QName(tag_name))?
    } else {
        Vec::new()
    };

    let mut writer_buf = Vec::new();
    let mut writer = Writer::new(&mut writer_buf);
    for event in &inner_events {
        writer.write_event(event.clone())?;
    }
    drop(writer);
    let inner_xml = String::from_utf8(writer_buf)
        .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?;

    // CORRECTED: This logic now correctly handles selections that result in a single
    // object/value as well as selections that result in an array of items.
    let items_to_iterate: Vec<&'a Value> = if let Some(first_val) = selected_values.get(0) {
        if let Some(arr) = first_val.as_array() {
            // The selection pointed to an array, so we iterate its members.
            arr.iter().collect()
        } else {
            // The selection pointed to a single item (or something else).
            // We "iterate" over the entire result set (which may be just one item).
            selected_values
        }
    } else {
        // The selection found nothing.
        Vec::new()
    };

    debug!(
        "  <{:?}> select='{}' found {} items.",
        String::from_utf8_lossy(tag_name),
        path,
        items_to_iterate.len()
    );

    for (i, item_context) in items_to_iterate.iter().enumerate() {
        debug!("  Processing item {} in for-each...", i);
        let mut inner_reader = Reader::from_str(&inner_xml);
        parse_nodes(parser, &mut inner_reader, item_context, processor)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_xsl_if<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let tag_name = b"xsl:if";
    let test = get_attr_owned_required(attributes, b"test", tag_name)?;
    let condition_met = !xpath::select(context, &test).is_empty();
    debug!("  <xsl:if test='{}'> -> {}", test, condition_met);
    if !is_empty {
        if condition_met {
            parse_nodes(parser, reader, context, processor)?;
        } else {
            reader.read_to_end_into(QName(tag_name), &mut Vec::new())?;
        }
    }
    Ok(())
}

// --- Page Structure Handlers ---

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_page_sequence<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let tag_name = b"page-sequence";
    let path = get_attr_owned_required(attributes, b"select", tag_name)?;
    let selected_values = xpath::select(context, &path);

    let inner_events = if !is_empty {
        capture_events_until_end(reader, QName(tag_name))?
    } else {
        Vec::new()
    };

    let mut writer_buf = Vec::new();
    let mut writer = Writer::new(&mut writer_buf);
    for event in &inner_events {
        writer.write_event(event.clone())?;
    }
    drop(writer);
    let inner_xml = String::from_utf8(writer_buf)
        .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?;

    // CORRECTED: This logic now correctly handles selections that result in a single
    // object/value (like `select="."`) as well as selections that result in an array.
    let items_to_iterate: Vec<&'a Value> = if path == "/" {
        vec![context]
    } else if let Some(first_val) = selected_values.get(0) {
        if let Some(arr) = first_val.as_array() {
            // The selection pointed to an array, so we iterate its members.
            arr.iter().collect()
        } else {
            // The selection pointed to a single item (or something else).
            // We iterate over the entire result set (which may be just one item).
            selected_values
        }
    } else {
        // The selection found nothing.
        Vec::new()
    };

    debug!(
        "  <{:?}> select='{}' found {} items.",
        String::from_utf8_lossy(tag_name),
        path,
        items_to_iterate.len()
    );

    for (i, item_context) in items_to_iterate.iter().enumerate() {
        debug!("  Processing item {} in page-sequence...", i);
        processor.process_event(Event::BeginPageSequenceItem {
            context: item_context,
        })?;
        let mut inner_reader = Reader::from_str(&inner_xml);
        parse_nodes(parser, &mut inner_reader, item_context, processor)?;
        processor.process_event(Event::EndPageSequenceItem)?;
    }
    Ok(())
}

// --- Layout Tag Handlers ---
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_container<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    processor.process_event(Event::StartContainer {
        style: style.map(Cow::Owned),
    })?;
    if !is_empty {
        parse_nodes(parser, reader, context, processor)?;
    }
    processor.process_event(Event::EndContainer)?;
    Ok(())
}

pub(super) fn handle_text<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let tag_name = b"text";
    let style = get_attr_owned_optional(attributes, b"style")?;
    let content = if !is_empty {
        parse_text_content(reader, QName(tag_name), context)?
    } else {
        String::new()
    };

    let rendered_content = if content.contains("{{") {
        parser
            .template_engine
            .render_template(&content, context)
            .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?
    } else {
        content
    };

    processor.process_event(Event::AddText {
        content: Cow::Owned(rendered_content),
        style: style.map(Cow::Owned),
    })?;
    Ok(())
}

pub(super) fn handle_rectangle<'a, R: DocumentRenderer<'a>>(
    attributes: &OwnedAttributes,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let style = get_attr_owned_optional(attributes, b"style")?;
    processor.process_event(Event::AddRectangle {
        style: style.map(Cow::Owned),
    })?;
    Ok(())
}

pub(super) fn handle_image<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let src = get_attr_owned_required(attributes, b"src", b"image")?;
    let style = get_attr_owned_optional(attributes, b"style")?;

    let rendered_src = if src.contains("{{") {
        parser
            .template_engine
            .render_template(&src, context)
            .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?
    } else {
        src
    };

    processor.process_event(Event::AddImage {
        src: Cow::Owned(rendered_src),
        style: style.map(Cow::Owned),
    })?;
    Ok(())
}

// --- Table Tag Handlers ---
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_table<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    if is_empty {
        return Ok(());
    }
    let style = get_attr_owned_optional(attributes, b"style")?;
    let mut columns = Vec::new();
    let mut has_header = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            XmlEvent::Start(child_e) => {
                let child_name = child_e.name();
                match child_name.as_ref() {
                    b"columns" => columns = parse_table_columns(reader, child_name)?,
                    b"header" => {
                        has_header = true;
                        processor.process_event(Event::StartTable {
                            style: style.clone().map(Cow::Owned),
                            columns: Cow::Owned(columns.clone()),
                        })?;
                        processor.process_event(Event::StartHeader)?;
                        parse_nodes(parser, reader, context, processor)?;
                        processor.process_event(Event::EndHeader)?;
                    }
                    b"tbody" => {
                        if !has_header {
                            processor.process_event(Event::StartTable {
                                style: style.clone().map(Cow::Owned),
                                columns: Cow::Owned(columns.clone()),
                            })?;
                        }
                        parse_nodes(parser, reader, context, processor)?;
                    }
                    _ => {
                        // Consume unknown tags within a table
                        reader.read_to_end_into(child_name, &mut Vec::new())?;
                    }
                }
            }
            XmlEvent::End(_) => break, // End of <table>
            XmlEvent::Eof => {
                return Err(PipelineError::TemplateParseError(
                    "Unexpected EOF in table".into(),
                ))
            }
            _ => (),
        }
        buf.clear();
    }
    processor.process_event(Event::EndTable)?;
    Ok(())
}

pub(super) fn handle_row<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    processor.process_event(Event::StartRow {
        context,
        row_style_prefix: None,
    })?;
    parser.row_column_index_stack.push(0);
    if !is_empty {
        parse_nodes(parser, reader, context, processor)?;
    }
    parser.row_column_index_stack.pop();
    processor.process_event(Event::EndRow)?;
    Ok(())
}

pub(super) fn handle_cell<'a, R: DocumentRenderer<'a>>(
    parser: &mut XsltTemplateParser<'a>,
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    processor: &mut StreamingLayoutProcessor<'a, R>,
) -> Result<(), PipelineError> {
    let tag_name = b"cell";
    let style_override = get_attr_owned_optional(attributes, b"style")?;
    let col_index = *parser
        .row_column_index_stack
        .last()
        .ok_or_else(|| PipelineError::TemplateParseError("<cell> outside <row>".into()))?;
    let content = if !is_empty {
        parse_text_content(reader, QName(tag_name), context)?
    } else {
        String::new()
    };

    let rendered_content = if content.contains("{{") {
        parser
            .template_engine
            .render_template(&content, context)
            .map_err(|e| PipelineError::TemplateParseError(e.to_string()))?
    } else {
        content
    };

    debug!("Adding cell at index {}: '{}'", col_index, rendered_content);
    processor.process_event(Event::AddCell {
        column_index: col_index,
        content: Cow::Owned(rendered_content),
        style_override,
    })?;
    if let Some(idx) = parser.row_column_index_stack.last_mut() {
        *idx += 1;
    }
    Ok(())
}