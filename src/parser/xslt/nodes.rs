// src/parser/xslt/nodes.rs
// src/parser/xslt/nodes.rs
use super::tags;
use super::util::OwnedAttributes;
use super::XsltTemplateParser;
use crate::error::PipelineError;
use crate::idf::IDFEvent;
use crate::parser::processor::LayoutProcessorProxy;
use log::debug;
use quick_xml::events::Event as XmlEvent;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde_json::Value;

/// Recursively parses XML nodes within a given data context, emitting layout events.
pub(super) async fn parse_nodes<'a>(
    parser: &mut XsltTemplateParser<'a>,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            XmlEvent::Start(e) => {
                let tag_name = e.name().as_ref().to_vec();
                let attributes = e
                    .attributes()
                    .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
                    .collect::<Result<OwnedAttributes, _>>()?;
                Box::pin(handle_tag(
                    parser, &tag_name, &attributes, false, reader, context, proxy,
                ))
                    .await?;
            }
            XmlEvent::Empty(e) => {
                let tag_name = e.name().as_ref().to_vec();
                let attributes = e
                    .attributes()
                    .map(|a| a.map(|attr| (attr.key.as_ref().to_vec(), attr.value.into_owned())))
                    .collect::<Result<OwnedAttributes, _>>()?;
                Box::pin(handle_tag(
                    parser, &tag_name, &attributes, true, reader, context, proxy,
                ))
                    .await?;
            }
            XmlEvent::End(_) => return Ok(()),
            XmlEvent::Eof => return Ok(()),
            _ => (),
        }
        buf.clear();
    }
}

/// Handles a start tag or empty tag by dispatching to the correct tag handler.
#[allow(clippy::too_many_arguments)]
async fn handle_tag<'a>(
    parser: &mut XsltTemplateParser<'a>,
    tag_name: &[u8],
    attributes: &OwnedAttributes,
    is_empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &'a Value,
    proxy: &mut LayoutProcessorProxy<'a>,
) -> Result<(), PipelineError> {
    let tag_name_str = String::from_utf8_lossy(tag_name);
    debug!("Handling tag: <{}> (is_empty: {})", tag_name_str, is_empty);

    match tag_name {
        // --- Control Flow & Data Tags ---
        b"xsl:for-each" => {
            tags::handle_xsl_for_each(parser, attributes, is_empty, reader, context, proxy)
                .await?
        }
        b"xsl:if" => tags::handle_xsl_if(parser, attributes, is_empty, reader, context, proxy).await?,

        // --- Page Structure Tags ---
        b"page-sequence" => {
            tags::handle_page_sequence(parser, attributes, is_empty, reader, context, proxy)
                .await?
        }
        b"page-break" => proxy.process_event(IDFEvent::ForcePageBreak).await?,

        // --- Layout Tags ---
        b"container" => {
            tags::handle_container(parser, attributes, is_empty, reader, context, proxy).await?
        }
        b"text" => tags::handle_text(parser, attributes, is_empty, reader, context, proxy).await?,
        b"rectangle" => tags::handle_rectangle(attributes, proxy).await?,
        b"image" => tags::handle_image(parser, attributes, context, proxy).await?,

        // --- Table Tags ---
        b"table" => tags::handle_table(parser, attributes, is_empty, reader, context, proxy).await?,
        b"row" => tags::handle_row(parser, is_empty, reader, context, proxy).await?,
        b"cell" => tags::handle_cell(parser, attributes, is_empty, reader, context, proxy).await?,

        // --- Structural Container Tags ---
        // These are container tags that should be recursed into.
        b"xsl:stylesheet" | b"xsl:template" | b"document" => {
            if !is_empty {
                // This is a structural tag, so we just parse its children.
                parse_nodes(parser, reader, context, proxy).await?;
            }
        }

        // --- Pre-processed/Ignored Tags ---
        // These tags were handled during stylesheet pre-parsing and should be skipped.
        b"xsl:attribute-set" | b"fo:layout-master-set" => {
            if !is_empty {
                // We need to consume the children of these tags so the parser can continue
                // without processing them.
                reader.read_to_end_into(QName(tag_name), &mut Vec::new())?;
            }
        }
        _ => return Err(PipelineError::UnknownXmlTag(tag_name_str.to_string())),
    }
    Ok(())
}