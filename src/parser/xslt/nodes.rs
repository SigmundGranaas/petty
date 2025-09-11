// src/parser/xslt/nodes.rs
use super::tags;
use super::util::OwnedAttributes;
use super::XsltTemplateParser;
use crate::error::PipelineError;
use crate::parser::processor::LayoutProcessorProxy;
use log::debug;
use quick_xml::events::Event as XmlEvent;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;

/// Recursively parses the children of the current XML node.
/// This function is called *after* a Start tag has been read, and it will
/// parse until the corresponding End tag is found.
pub(super) async fn parse_nodes<'a>(
    parser: &mut XsltTemplateParser<'a>,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let mut buf = Vec::new();
    let mut depth = 0;

    loop {
        match reader.read_event_into(&mut buf)? {
            XmlEvent::Start(e) => {
                depth += 1;
                let tag_name = e.name().as_ref().to_vec();
                let attributes = e
                    .attributes()
                    .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
                    .collect::<Result<OwnedAttributes, _>>()?;

                let consumed_end_tag = Box::pin(handle_tag(
                    parser, &tag_name, &attributes, false, reader, context, proxy,
                ))
                    .await?;

                // If the handler consumed the corresponding end tag, we must decrement the
                // depth counter here to keep it balanced, since the End event will
                // never be seen by this loop.
                if consumed_end_tag {
                    depth -= 1;
                }
            }
            XmlEvent::Empty(e) => {
                let tag_name = e.name().as_ref().to_vec();
                let attributes = e
                    .attributes()
                    .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
                    .collect::<Result<OwnedAttributes, _>>()?;
                // Since this is a self-closing tag, depth is not incremented, and we
                // don't care about the return value.
                Box::pin(handle_tag(
                    parser, &tag_name, &attributes, true, reader, context, proxy,
                ))
                    .await?;
            }
            XmlEvent::End(_) => {
                if depth == 0 {
                    // We found the matching End tag for the node that this
                    // `parse_nodes` call was responsible for. Time to return.
                    return Ok(());
                }
                depth -= 1;
            }
            XmlEvent::Eof => {
                // If we reach EOF and depth is not zero, it means a tag was left open.
                if depth > 0 {
                    return Err(PipelineError::TemplateParseError(
                        "Unexpected EOF - unclosed tag found.".into(),
                    ));
                }
                // Otherwise, we've successfully parsed to the end of a fragment.
                return Ok(());
            }
            _ => (),
        }
        buf.clear();
    }
}

/// Dispatches to the correct tag handler.
/// Returns a boolean indicating if the handler consumed the element's end tag.
#[allow(clippy::too_many_arguments)]
async fn handle_tag<'a>(
    parser: &mut XsltTemplateParser<'a>,
    tag_name: &[u8],
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<bool, PipelineError> {
    let tag_name_str = String::from_utf8_lossy(tag_name);
    debug!("Handling tag: <{}> (is_empty: {})", tag_name_str, is_empty);

    match tag_name {
        // --- Control Flow & Data Tags ---
        b"xsl:for-each" => tags::handle_xsl_for_each(parser, attributes, reader, context, proxy).await?,
        b"xsl:if" => tags::handle_xsl_if(parser, attributes, reader, context, proxy).await?,

        // --- Page Structure Tags ---
        b"page-sequence" => tags::handle_page_sequence(parser, attributes, reader, context, proxy).await?,
        b"page-break" => proxy.process_event(crate::idf::IDFEvent::ForcePageBreak).await?,
        b"br" => proxy.process_event(crate::idf::IDFEvent::AddLineBreak).await?,

        // --- Layout Tags ---
        b"container" => tags::handle_container(parser, attributes, !is_empty, reader, context, proxy).await?,
        b"text" => tags::handle_text(parser, attributes, !is_empty, reader, context, proxy).await?,
        b"rectangle" => tags::handle_rectangle(attributes, proxy).await?,
        b"image" => tags::handle_image(parser, attributes, context, proxy).await?,

        // --- Semantic & Interactive Tags ---
        b"link" => tags::handle_link(parser, attributes, !is_empty, reader, context, proxy).await?,
        b"strong" | b"b" => tags::handle_inline(parser, "bold", !is_empty, reader, context, proxy).await?,
        b"em" | b"i" => tags::handle_inline(parser, "italic", !is_empty, reader, context, proxy).await?,

        // --- Table Tags ---
        b"table" => tags::handle_table(parser, attributes, reader, context, proxy).await?,
        b"tbody" | b"header" => { if !is_empty { parse_nodes(parser, reader, context, proxy).await? } }
        b"row" => tags::handle_row(parser, !is_empty, reader, context, proxy).await?,
        b"cell" => tags::handle_cell(parser, attributes, !is_empty, reader, context, proxy).await?,

        // --- Structural & Ignored Tags ---
        b"xsl:stylesheet" | b"xsl:template" | b"document" => { if !is_empty { parse_nodes(parser, reader, context, proxy).await? } }
        b"xsl:attribute-set" | b"fo:layout-master-set" | b"columns" | b"column" => { if !is_empty { reader.read_to_end_into(QName(tag_name), &mut Vec::new())?; } }

        _ => return Err(PipelineError::UnknownXmlTag(tag_name_str.to_string())),
    }

    // A handler for a non-empty tag is assumed to consume its corresponding end tag.
    // Tags that are always empty (like <br> or <rectangle>) don't have an end tag
    // to consume, so for them, we return false.
    let consumed_end_tag = match tag_name {
        b"rectangle" | b"image" | b"page-break" | b"br" => false,
        _ => !is_empty,
    };

    Ok(consumed_end_tag)
}