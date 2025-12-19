//! Handlers for `<xsl:template>` and related instructions.

use crate::parser::xslt::ast::{NamedTemplate, PreparsedTemplate, TemplateRule, XsltInstruction};
use crate::parser::xslt::compiler::{BuilderState, CompilerBuilder};
use crate::parser::xslt::pattern;
use crate::parser::xslt::util::{get_attr_owned_optional, get_attr_owned_required, OwnedAttributes};
use crate::parser::{ParseError};

impl CompilerBuilder {
    pub(crate) fn handle_template_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        if let Some(role_name) = get_attr_owned_optional(&attrs, b"petty:role")? {
            self.state_stack.push(BuilderState::RoleTemplate { role_name, attrs });
        } else if let Some(template_name) = get_attr_owned_optional(&attrs, b"name")? {
            self.state_stack.push(BuilderState::NamedTemplate {
                name: template_name,
                params: vec![],
            });
        } else {
            // This is a rule template, ensure it has a match attribute.
            let _ = get_attr_owned_required(&attrs, b"match", b"xsl:template", pos, source)?;
            self.state_stack.push(BuilderState::Template(attrs));
        }
        Ok(())
    }

    pub(crate) fn handle_template_end(
        &mut self,
        current_state: BuilderState,
        body: Vec<XsltInstruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        match current_state {
            BuilderState::Template(attrs) => {
                let match_str = get_attr_owned_required(&attrs, b"match", b"xsl:template", pos, source)?;
                let pattern = pattern::parse(&match_str)?;
                let mode = get_attr_owned_optional(&attrs, b"mode")?;
                let priority = get_attr_owned_optional(&attrs, b"priority")?
                    .map(|p_str| {
                        p_str
                            .parse::<f64>()
                            .map_err(|e| ParseError::FloatParse(e, p_str.clone()))
                    })
                    .transpose()?
                    .unwrap_or_else(|| calculate_default_priority(&match_str));
                let rule = TemplateRule {
                    pattern,
                    priority,
                    mode: mode.clone(),
                    body: PreparsedTemplate(body),
                };
                self.template_rules.entry(mode).or_default().push(rule);
            }
            BuilderState::NamedTemplate { name, params } => {
                let template = NamedTemplate {
                    params,
                    body: PreparsedTemplate(body),
                };
                self.named_templates.insert(name, std::sync::Arc::new(template));
            }
            BuilderState::RoleTemplate { role_name, attrs } => {
                // Generate a unique mode for this role template.
                let unique_mode = format!("__petty_role_{}", role_name);
                self.role_template_modes.insert(role_name, unique_mode.clone());

                // A role template acts like a regular template rule, but with a specific mode.
                // It defaults to matching the root node if no `match` is provided.
                let match_str = get_attr_owned_optional(&attrs, b"match")?.unwrap_or_else(|| "/".to_string());
                let pattern = pattern::parse(&match_str)?;

                let rule = TemplateRule {
                    pattern,
                    priority: 1.0, // Role templates should have high priority if a match is specified.
                    mode: Some(unique_mode),
                    body: PreparsedTemplate(body),
                };
                self.template_rules.entry(rule.mode.clone()).or_default().push(rule);
            }
            _ => {} // Should not happen
        }
        Ok(())
    }

    pub(crate) fn handle_call_template_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), ParseError> {
        let name = get_attr_owned_required(&attrs, b"name", b"xsl:call-template", pos, source)?;
        self.state_stack.push(BuilderState::CallTemplate {
            name,
            params: vec![],
        });
        Ok(())
    }

    pub(crate) fn handle_call_template_end(
        &mut self,
        current_state: BuilderState,
    ) -> Result<(), ParseError> {
        if let BuilderState::CallTemplate { name, params } = current_state {
            let instr = XsltInstruction::CallTemplate { name, params };
            if let Some(parent) = self.instruction_stack.last_mut() {
                parent.push(instr);
            }
        }
        Ok(())
    }
}

fn calculate_default_priority(pattern_str: &str) -> f64 {
    // This is a simplified version of XSLT's priority rules
    match pattern_str {
        "/" => -0.5,
        p if p.contains('*') => -0.5,
        "text()" | "node()" => -0.25,
        p if p.contains('/') && !p.starts_with('/') => 0.0,
        p if p.contains(':') => 0.0,
        _ => 0.0, // Simple name tests
    }
}
