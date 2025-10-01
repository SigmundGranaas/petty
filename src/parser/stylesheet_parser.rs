//! A structured, recursive-descent parser for XSLT stylesheet definitions.
//!
//! This parser is responsible for consuming an XSLT file and extracting all
//! stylesheet-related information, such as `<xsl:attribute-set>` and page layout
//! definitions, into a structured `Stylesheet` object. It uses a robust,
//! state-encapsulated design to replace the previous monolithic state machine.

use crate::core::style::stylesheet::{ElementStyle, PageLayout, Stylesheet};
use crate::parser::error::{Location, ParseError};
use crate::parser::style::{apply_style_property, parse_page_size};
use crate::parser::style_parsers::{parse_length, parse_shorthand_margins, run_parser};
use crate::parser::xslt::util::get_owned_attributes;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::Reader;
use std::str::from_utf8;
use std::sync::Arc;

/// The main parser struct, encapsulating the XML reader state.
pub struct XsltParser<'a> {
    content: &'a str,
    reader: Reader<&'a [u8]>,
    buf: Vec<u8>,
}

impl<'a> XsltParser<'a> {
    /// Creates a new parser for the given XSLT content.
    pub fn new(content: &'a str) -> Self {
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);
        Self {
            content,
            reader,
            buf: Vec::new(),
        }
    }

    /// The main entry point. Parses the entire document into a `Stylesheet`.
    pub fn parse(mut self) -> Result<Stylesheet, ParseError> {
        loop {
            match self.read_event()? {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:stylesheet" => {
                    return self.parse_stylesheet_content();
                }
                XmlEvent::Eof => {
                    return Err(ParseError::TemplateStructure {
                        message: "Could not find root <xsl:stylesheet> element.".to_string(),
                        location: self.get_current_location(),
                    })
                }
                _ => (), // Ignore anything outside the root tag
            }
        }
    }

    /// Parses the content between `<xsl:stylesheet>` and `</xsl:stylesheet>`.
    fn parse_stylesheet_content(&mut self) -> Result<Stylesheet, ParseError> {
        let mut stylesheet = Stylesheet::default();
        loop {
            let event = self.read_event()?; // Read event once at the top of the loop
            match event {
                XmlEvent::Start(e) => {
                    let start_tag = e.clone(); // Clone to own the data for this iteration
                    match start_tag.name().as_ref() {
                        b"fo:simple-page-master" => {
                            stylesheet.page = self.parse_simple_page_master(&start_tag)?;
                            self.skip_element(start_tag.name())?;
                        }
                        b"xsl:attribute-set" => {
                            let (name, style) = self.parse_attribute_set(&start_tag)?;
                            stylesheet.styles.insert(name, Arc::new(style));
                        }
                        _ => self.skip_element(start_tag.name())?,
                    }
                }
                XmlEvent::End(e) if e.name().as_ref() == b"xsl:stylesheet" => break,
                XmlEvent::Eof => {
                    return Err(ParseError::TemplateStructure {
                        message: "Unexpected end of file while parsing <xsl:stylesheet>.".to_string(),
                        location: self.get_current_location(),
                    });
                }
                _ => (),
            }
        }
        Ok(stylesheet)
    }

    /// Parses an `<xsl:attribute-set>` and its child `<xsl:attribute>` tags.
    fn parse_attribute_set(&mut self, start_tag: &BytesStart) -> Result<(String, ElementStyle), ParseError> {
        let location = self.get_current_location();
        let attrs = get_owned_attributes(start_tag)?;
        let name = attrs.iter().find(|(k, _)| k.as_slice() == b"name")
            .and_then(|(_, v)| from_utf8(v).ok())
            .ok_or_else(|| ParseError::TemplateStructure {
                message: "<xsl:attribute-set> is missing the required 'name' attribute.".to_string(),
                location,
            })?
            .to_string();

        let mut style = ElementStyle::default();
        loop {
            let event = self.read_event()?;
            match event {
                XmlEvent::Start(e) if e.name().as_ref() == b"xsl:attribute" => {
                    let attr_location = self.get_current_location(); // Get location before mutable borrow
                    let (prop, val) = self.parse_attribute_content(&e)?;
                    apply_style_property(&mut style, &prop, &val).map_err(|e| match e {
                        ParseError::Nom(msg) => ParseError::InvalidStyleProperty {
                            property: prop,
                            value: val,
                            message: msg,
                            location: attr_location,
                        },
                        _ => e,
                    })?;
                }
                XmlEvent::End(e) if e.name() == start_tag.name() => break,
                XmlEvent::Eof => return Err(ParseError::TemplateStructure {
                    message: format!("Unexpected EOF while parsing attribute-set '{}'.", name),
                    location: self.get_current_location(),
                }),
                _ => (),
            }
        }
        Ok((name, style))
    }

    /// Parses the name and content of an `<xsl:attribute>`.
    fn parse_attribute_content(&mut self, start_tag: &BytesStart) -> Result<(String, String), ParseError> {
        let mut value = String::new();
        let attrs = get_owned_attributes(start_tag)?;
        let name = attrs.iter().find(|(k, _)| k.as_slice() == b"name")
            .and_then(|(_, v)| from_utf8(v).ok())
            .ok_or_else(|| ParseError::TemplateStructure {
                message: "<xsl:attribute> is missing 'name' attribute.".to_string(),
                location: self.get_current_location(),
            })?.to_string();

        if !start_tag.is_empty() {
            loop {
                match self.read_event()? {
                    XmlEvent::Text(e) => {
                        value = e.unescape()?.into_owned();
                    }
                    XmlEvent::End(e) if e.name() == start_tag.name() => break,
                    XmlEvent::Eof => return Err(ParseError::TemplateStructure {
                        message: "Unexpected EOF while parsing attribute content.".to_string(),
                        location: self.get_current_location(),
                    }),
                    _ => {}
                }
            }
        } else {
            // Self-closing tags might have content in a `select` attribute.
            if let Some(select_val) = attrs.iter().find(|(k, _)| k.as_slice() == b"select") {
                value = from_utf8(&select_val.1)?.to_string();
            }
        }
        Ok((name, value))
    }

    /// Parses a `<fo:simple-page-master>` tag into a `PageLayout`.
    fn parse_simple_page_master(&self, start_tag: &BytesStart) -> Result<PageLayout, ParseError> {
        let attrs = get_owned_attributes(start_tag)?;
        let mut page = PageLayout::default();

        for (key, val_bytes) in &attrs {
            let key_str = from_utf8(key)?;
            let val_str = from_utf8(val_bytes)?;
            match key_str {
                "master-name" => page.name = Some(val_str.to_string()),
                "page-width" => page.size.set_width(run_parser(parse_length, val_str)?),
                "page-height" => page.size.set_height(run_parser(parse_length, val_str)?),
                "size" => page.size = parse_page_size(val_str)?,
                "margin" => page.margins = Some(parse_shorthand_margins(val_str)?),
                "margin-top" => page.margins.get_or_insert_with(Default::default).top = run_parser(parse_length, val_str)?,
                "margin-right" => page.margins.get_or_insert_with(Default::default).right = run_parser(parse_length, val_str)?,
                "margin-bottom" => page.margins.get_or_insert_with(Default::default).bottom = run_parser(parse_length, val_str)?,
                "margin-left" => page.margins.get_or_insert_with(Default::default).left = run_parser(parse_length, val_str)?,
                _ => log::warn!("Unknown attribute on <fo:simple-page-master>: '{}'", key_str),
            }
        }
        Ok(page)
    }

    /// Consumes XML events until the corresponding end tag for `start_name` is found.
    fn skip_element(&mut self, _start_name: quick_xml::name::QName<'_>) -> Result<(), ParseError> {
        let mut depth = 1;
        while depth > 0 {
            match self.read_event()? {
                XmlEvent::Start(_) => depth += 1,
                XmlEvent::End(_) => depth -= 1,
                XmlEvent::Eof => break, // Let the caller handle the unexpected EOF
                _ => (), // Ignore Text, Empty, etc. within the skipped element
            }
        }
        Ok(())
    }

    /// Reads the next XML event and enriches any error with location info.
    fn read_event(&mut self) -> Result<XmlEvent<'static>, ParseError> {
        self.buf.clear();
        let location = self.get_current_location();
        self.reader
            .read_event_into(&mut self.buf)
            .map(|e| e.into_owned())
            .map_err(|source| ParseError::Xml { source, location })
    }

    /// Calculates the current line and column number from the reader's position.
    fn get_current_location(&self) -> Location {
        let pos = self.reader.buffer_position();
        let (line, col) = super::xslt::util::get_line_col_from_pos(self.content, pos.try_into().unwrap());
        Location { line, col }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::style::color::Color;

    #[test]
    fn test_parse_stylesheet_with_templates_and_attributes() {
        let xslt = r#"
        <xsl:stylesheet version="1.0"
            xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
            xmlns:fo="http://www.w3.org/1999/XSL/Format">

            <xsl:attribute-set name="title-style">
                <xsl:attribute name="font-size">18pt</xsl:attribute>
                <xsl:attribute name="font-weight">bold</xsl:attribute>
            </xsl:attribute-set>

            <xsl:template match="/">
                <!-- This content should be skipped -->
                <fo:block>
                    <xsl:value-of select="."/>
                </fo:block>
            </xsl:template>

            <xsl:attribute-set name="red-text">
                <xsl:attribute name="color">#FF0000</xsl:attribute>
            </xsl:attribute-set>

        </xsl:stylesheet>
        "#;

        let parser = XsltParser::new(xslt);
        let stylesheet = parser.parse().unwrap();

        assert_eq!(stylesheet.styles.len(), 2, "Should have parsed two attribute-sets");

        let title_style = stylesheet.styles.get("title-style").unwrap();
        assert_eq!(title_style.font_size, Some(18.0));
        assert_eq!(title_style.font_weight.as_ref().unwrap(), &crate::core::style::font::FontWeight::Bold);

        let red_style = stylesheet.styles.get("red-text").unwrap();
        assert_eq!(red_style.color, Some(Color { r: 255, g: 0, b: 0, a: 1.0 }));
    }

    #[test]
    fn test_unclosed_stylesheet_tag() {
        let xslt = r#"<xsl:stylesheet version="1.0">
            <xsl:attribute-set name="test"></xsl:attribute-set>
            <!-- Missing closing stylesheet tag -->
        "#;
        let parser = XsltParser::new(xslt);
        let result = parser.parse();
        assert!(result.is_err());
        let err_string = result.unwrap_err().to_string();
        assert!(err_string.contains("Unexpected end of file while parsing <xsl:stylesheet>"));
    }
}