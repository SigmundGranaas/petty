pub(crate) mod control_flow;
pub(crate) mod loops;
pub(crate) mod streaming;
pub(crate) mod stylesheet;
pub(crate) mod variables;
pub(crate) mod xslt3_elements;

use crate::ast::{Avt3, Avt3Part, PreparsedTemplate, Xslt3Instruction};
use crate::compiler::{BuilderState3, CompilerBuilder3, OwnedAttributes};
use crate::error::Xslt3Error;
use petty_xslt::ast::AttributeValueTemplate;
use quick_xml::events::BytesEnd;
use std::str::from_utf8;

impl CompilerBuilder3 {
    pub(crate) fn handle_literal_element_end(
        &mut self,
        e: &BytesEnd,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::InstructionBody(attrs) = current_state {
            let styles = self.resolve_styles(&attrs)?;
            let non_style_attrs = self.get_non_style_attributes(&attrs)?;
            let shadow_attrs = self.get_shadow_attributes(&attrs)?;
            let use_attribute_sets = self.get_use_attribute_sets(&attrs)?;
            let tag_name = self.apply_namespace_aliases(e.name().as_ref());
            let instr = Xslt3Instruction::ContentTag {
                tag_name,
                styles,
                attrs: non_style_attrs,
                shadow_attrs,
                use_attribute_sets,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn apply_namespace_aliases(&self, name: &[u8]) -> Vec<u8> {
        if self.namespace_aliases.is_empty() {
            return name.to_vec();
        }

        let name_str = match from_utf8(name) {
            Ok(s) => s,
            Err(_) => return name.to_vec(),
        };

        if let Some((prefix, local)) = name_str.split_once(':') {
            for alias in &self.namespace_aliases {
                if alias.stylesheet_prefix == prefix {
                    let result_prefix = if alias.result_prefix == "#default" {
                        return local.as_bytes().to_vec();
                    } else {
                        &alias.result_prefix
                    };
                    return format!("{}:{}", result_prefix, local).into_bytes();
                }
            }
        }

        name.to_vec()
    }

    pub(crate) fn get_use_attribute_sets(
        &self,
        attrs: &OwnedAttributes,
    ) -> Result<Vec<String>, Xslt3Error> {
        for (key, value) in attrs {
            if key == b"xsl:use-attribute-sets" || key == b"use-attribute-sets" {
                let value_str = from_utf8(value).map_err(|e| Xslt3Error::parse(e.to_string()))?;
                return Ok(value_str.split_whitespace().map(String::from).collect());
            }
        }
        Ok(Vec::new())
    }

    pub(crate) fn get_non_style_attributes(
        &self,
        attrs: &OwnedAttributes,
    ) -> Result<std::collections::HashMap<String, Avt3>, Xslt3Error> {
        let (regular, _shadow) = self.split_attributes(attrs)?;
        Ok(regular)
    }

    pub(crate) fn get_shadow_attributes(
        &self,
        attrs: &OwnedAttributes,
    ) -> Result<Vec<crate::ast::ShadowAttribute>, Xslt3Error> {
        let (_regular, shadow) = self.split_attributes(attrs)?;
        Ok(shadow)
    }

    fn split_attributes(
        &self,
        attrs: &OwnedAttributes,
    ) -> Result<
        (
            std::collections::HashMap<String, Avt3>,
            Vec<crate::ast::ShadowAttribute>,
        ),
        Xslt3Error,
    > {
        use std::collections::HashMap;

        const STYLE_ATTRS: &[&[u8]] = &[
            b"style",
            b"use-attribute-sets",
            b"id",
            b"class",
            b"font-family",
            b"font-size",
            b"font-weight",
            b"font-style",
            b"color",
            b"background-color",
            b"margin",
            b"margin-top",
            b"margin-bottom",
            b"margin-left",
            b"margin-right",
            b"padding",
            b"padding-top",
            b"padding-bottom",
            b"padding-left",
            b"padding-right",
            b"border",
            b"text-align",
            b"line-height",
            b"width",
            b"height",
            b"display",
            b"flex-direction",
            b"justify-content",
            b"align-items",
            b"expand-text",
        ];

        let mut regular = HashMap::new();
        let mut shadow = Vec::new();

        for (key, value) in attrs {
            if STYLE_ATTRS.contains(&key.as_slice()) {
                continue;
            }

            let key_str = from_utf8(key).map_err(|e| Xslt3Error::parse(e.to_string()))?;
            let value_str = from_utf8(value).map_err(|e| Xslt3Error::parse(e.to_string()))?;

            if let Some(real_name) = key_str.strip_prefix('_') {
                let name_avt = self.parse_avt3(real_name)?;
                let value_avt = self.parse_avt3(value_str)?;
                shadow.push(crate::ast::ShadowAttribute {
                    name: name_avt,
                    value: value_avt,
                });
            } else {
                let avt = self.parse_avt3(value_str)?;
                regular.insert(key_str.to_string(), avt);
            }
        }
        Ok((regular, shadow))
    }

    pub(crate) fn parse_avt(&self, s: &str) -> Result<AttributeValueTemplate, Xslt3Error> {
        if !s.contains('{') {
            return Ok(AttributeValueTemplate::Static(s.to_string()));
        }

        let mut parts = Vec::new();
        let mut current_static = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        current_static.push('{');
                    } else {
                        if !current_static.is_empty() {
                            parts.push(petty_xslt::ast::AvtPart::Static(std::mem::take(
                                &mut current_static,
                            )));
                        }
                        let mut expr_str = String::new();
                        let mut depth = 1;
                        for ec in chars.by_ref() {
                            match ec {
                                '{' => {
                                    depth += 1;
                                    expr_str.push(ec);
                                }
                                '}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                    expr_str.push(ec);
                                }
                                _ => expr_str.push(ec),
                            }
                        }
                        let expr = petty_xpath1::parse_expression(&expr_str).map_err(|e| {
                            Xslt3Error::parse(format!("Failed to parse AVT expression: {}", e))
                        })?;
                        parts.push(petty_xslt::ast::AvtPart::Dynamic(expr));
                    }
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        current_static.push('}');
                    } else {
                        current_static.push('}');
                    }
                }
                _ => current_static.push(c),
            }
        }

        if !current_static.is_empty() {
            parts.push(petty_xslt::ast::AvtPart::Static(current_static));
        }

        if parts.len() == 1
            && let Some(petty_xslt::ast::AvtPart::Static(s)) = parts.first()
        {
            Ok(AttributeValueTemplate::Static(s.clone()))
        } else {
            Ok(AttributeValueTemplate::Dynamic(parts))
        }
    }

    pub(crate) fn parse_avt3(&self, s: &str) -> Result<Avt3, Xslt3Error> {
        if !s.contains('{') {
            return Ok(Avt3::Static(s.to_string()));
        }

        let mut parts = Vec::new();
        let mut current_static = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        current_static.push('{');
                    } else {
                        if !current_static.is_empty() {
                            parts.push(Avt3Part::Static(std::mem::take(&mut current_static)));
                        }
                        let mut expr_str = String::new();
                        let mut depth = 1;
                        for ec in chars.by_ref() {
                            match ec {
                                '{' => {
                                    depth += 1;
                                    expr_str.push(ec);
                                }
                                '}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                    expr_str.push(ec);
                                }
                                _ => expr_str.push(ec),
                            }
                        }
                        let expr = petty_xpath31::parse_expression(&expr_str).map_err(|e| {
                            Xslt3Error::parse(format!("Failed to parse AVT expression: {}", e))
                        })?;
                        parts.push(Avt3Part::Dynamic(expr));
                    }
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        current_static.push('}');
                    } else {
                        current_static.push('}');
                    }
                }
                _ => current_static.push(c),
            }
        }

        if !current_static.is_empty() {
            parts.push(Avt3Part::Static(current_static));
        }

        if parts.len() == 1
            && let Some(Avt3Part::Static(s)) = parts.first()
        {
            Ok(Avt3::Static(s.clone()))
        } else {
            Ok(Avt3::Dynamic(parts))
        }
    }

    pub(crate) fn handle_text_end(
        &mut self,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let Some(parent) = self.instruction_stack.last_mut() {
            for instr in body {
                parent.push(instr);
            }
        }
        Ok(())
    }

    pub(crate) fn handle_copy_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::InstructionBody(attrs) = current_state {
            let styles = self.resolve_styles(&attrs)?;
            let instr = Xslt3Instruction::Copy {
                styles,
                body: PreparsedTemplate(body),
            };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }
}
