// FILE: /home/sigmund/RustroverProjects/petty/src/parser/xslt/compiler_handlers/stylesheet.rs
// FILE: src/parser/xslt/compiler_handlers/stylesheet.rs
//! Handlers for top-level XSLT stylesheet elements and simple literal instructions.

use crate::core::style::stylesheet::PageLayout;
use crate::parser::style::parse_page_size;
use crate::parser::style_parsers::{parse_length, parse_shorthand_margins, run_parser};
use crate::parser::xslt::ast::XsltInstruction;
use crate::parser::xslt::compiler::{BuilderState, CompilerBuilder};
use crate::parser::{Location, ParseError};
use crate::parser::xslt::util::{get_attr_owned_optional, get_attr_owned_required, OwnedAttributes};
use quick_xml::events::BytesStart;
use std::str::from_utf8;
use crate::parser::xslt::xpath;

impl CompilerBuilder {
    pub(crate) fn handle_stylesheet_start(&mut self) {
        self.state_stack.push(BuilderState::Stylesheet);
    }

    pub(crate) fn handle_text_start(&mut self) {
        self.state_stack.push(BuilderState::XslText);
    }

    pub(crate) fn handle_text_end(&mut self, body: Vec<XsltInstruction>) -> Result<(), ParseError> {
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.extend(body);
        }
        Ok(())
    }

    pub(crate) fn handle_simple_page_master(
        &mut self,
        attrs: OwnedAttributes,
    ) -> Result<(), ParseError> {
        let mut page = PageLayout::default();
        let master_name = get_attr_owned_optional(&attrs, b"master-name")?;
        for (key, val_bytes) in &attrs {
            let key_str = from_utf8(key)?;
            let val_str = from_utf8(val_bytes)?;
            match key_str {
                "master-name" => {}
                "page-width" => page.size.set_width(run_parser(parse_length, val_str)?),
                "page-height" => page.size.set_height(run_parser(parse_length, val_str)?),
                "size" => page.size = parse_page_size(val_str)?,
                "margin" => page.margins = Some(parse_shorthand_margins(val_str)?),
                _ => {}
            }
        }
        self.stylesheet.page_masters.insert(
            master_name.unwrap_or_else(|| "default".to_string()),
            page,
        );
        Ok(())
    }

    pub(crate) fn handle_empty_literal_result_element(
        &self,
        e: &BytesStart,
        attrs: OwnedAttributes,
        location: Location,
    ) -> Result<XsltInstruction, ParseError> {
        let styles = self.resolve_styles(&attrs, location)?;
        let non_style_attrs = super::super::util::get_non_style_attributes(&attrs)?;
        Ok(XsltInstruction::EmptyTag {
            tag_name: e.name().as_ref().to_vec(),
            styles,
            attrs: non_style_attrs,
        })
    }

    // --- Handlers for simple, empty instructions ---

    pub(crate) fn handle_value_of(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let instr = XsltInstruction::ValueOf {
            select: xpath::parse_expression(&get_attr_owned_required(
                &attrs,
                b"select",
                b"xsl:value-of",
                pos,
                source,
            )?)?,
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_copy_of(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let instr = XsltInstruction::CopyOf {
            select: xpath::parse_expression(&get_attr_owned_required(
                &attrs,
                b"select",
                b"xsl:copy-of",
                pos,
                source,
            )?)?,
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_page_break(&mut self, attrs: OwnedAttributes) -> Result<(), ParseError> {
        let instr = XsltInstruction::PageBreak {
            master_name: get_attr_owned_optional(&attrs, b"master-name")?
                .map(|s| crate::parser::xslt::util::parse_avt(&s))
                .transpose()?,
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}