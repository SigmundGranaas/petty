//! Handlers for top-level XSLT stylesheet elements and simple literal instructions.

use crate::ast::XsltInstruction;
use crate::compiler::{BuilderState, CompilerBuilder};
use crate::error::{Location, XsltError};
use crate::util::{OwnedAttributes, get_attr_owned_optional, get_attr_owned_required};
use petty_style::parsers::{parse_length, parse_page_size, parse_shorthand_margins, run_parser};
use petty_style::stylesheet::PageLayout;
use quick_xml::events::BytesStart;
use std::str::from_utf8;

impl CompilerBuilder {
    pub(crate) fn handle_stylesheet_start(&mut self) {
        self.state_stack.push(BuilderState::Stylesheet);
    }

    pub(crate) fn handle_text_start(&mut self) {
        self.state_stack.push(BuilderState::XslText);
    }

    pub(crate) fn handle_text_end(&mut self, body: Vec<XsltInstruction>) -> Result<(), XsltError> {
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.extend(body);
        }
        Ok(())
    }

    pub(crate) fn handle_simple_page_master(
        &mut self,
        attrs: OwnedAttributes,
    ) -> Result<(), XsltError> {
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
        self.stylesheet
            .page_masters
            .insert(master_name.unwrap_or_else(|| "default".to_string()), page);
        Ok(())
    }

    pub(crate) fn handle_empty_literal_result_element(
        &mut self,
        e: &BytesStart,
        attrs: OwnedAttributes,
        location: Location,
    ) -> Result<XsltInstruction, XsltError> {
        let styles = self.resolve_styles(&attrs, location)?;
        let non_style_attrs = crate::util::get_non_style_attributes(self, &attrs)?;
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
    ) -> Result<(), XsltError> {
        let select_str = get_attr_owned_required(&attrs, b"select", b"xsl:value-of", pos, source)?;
        let instr = XsltInstruction::ValueOf {
            select: self.parse_xpath_and_detect_features(&select_str)?,
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
    ) -> Result<(), XsltError> {
        let select_str = get_attr_owned_required(&attrs, b"select", b"xsl:copy-of", pos, source)?;
        let instr = XsltInstruction::CopyOf {
            select: self.parse_xpath_and_detect_features(&select_str)?,
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_page_break(&mut self, attrs: OwnedAttributes) -> Result<(), XsltError> {
        let instr = XsltInstruction::PageBreak {
            master_name: get_attr_owned_optional(&attrs, b"master-name")?
                .map(|s| crate::util::parse_avt(self, &s))
                .transpose()?,
        };
        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }
}
