use crate::ast::{
    Function3, NamedTemplate3, Pattern3, PreparsedTemplate, TemplateRule3, Visibility,
    Xslt3Instruction,
};
use crate::compiler::{
    BuilderState3, CompilerBuilder3, OwnedAttributes, get_attr_optional, get_attr_required,
};
use crate::error::Xslt3Error;
use petty_style::parsers::{parse_length, parse_page_size, parse_shorthand_margins, run_parser};
use petty_style::stylesheet::PageLayout;
use std::str::from_utf8;
use std::sync::Arc;

impl CompilerBuilder3 {
    pub(crate) fn handle_stylesheet_start(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        if let Some(version) = get_attr_optional(attrs, b"version")? {
            self.version = version;
        }
        if let Some(mode) = get_attr_optional(attrs, b"default-mode")? {
            self.default_mode = Some(mode);
        }
        if let Some(expand) = get_attr_optional(attrs, b"expand-text")? {
            self.expand_text = expand == "yes" || expand == "true";
        }
        Ok(())
    }

    pub(crate) fn handle_package_start(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        if let Some(version) = get_attr_optional(attrs, b"version")? {
            self.version = version;
        }
        if let Some(name) = get_attr_optional(attrs, b"name")? {
            self.features.package_name = Some(name);
        }
        if let Some(pkg_version) = get_attr_optional(attrs, b"package-version")? {
            self.features.package_version = Some(pkg_version);
        }
        if let Some(mode) = get_attr_optional(attrs, b"default-mode")? {
            self.default_mode = Some(mode);
        }
        if let Some(expand) = get_attr_optional(attrs, b"expand-text")? {
            self.expand_text = expand == "yes" || expand == "true";
        }
        Ok(())
    }

    pub(crate) fn handle_template_start(
        &mut self,
        attrs: OwnedAttributes,
        _pos: usize,
        _source: &str,
    ) -> Result<(), Xslt3Error> {
        if let Some(name) = get_attr_optional(&attrs, b"name")? {
            self.state_stack.push(BuilderState3::NamedTemplate {
                name,
                params: Vec::new(),
            });
        } else {
            self.state_stack.push(BuilderState3::Template(attrs));
        }
        Ok(())
    }

    pub(crate) fn handle_template_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        match current_state {
            BuilderState3::Template(attrs) => {
                let match_str = get_attr_required(&attrs, b"match", b"xsl:template", pos, source)?;
                let priority = get_attr_optional(&attrs, b"priority")?
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.5);
                let mode = get_attr_optional(&attrs, b"mode")?;
                let visibility = get_attr_optional(&attrs, b"visibility")?
                    .map(|s| self.parse_visibility(&s))
                    .unwrap_or_default();

                let rule = TemplateRule3 {
                    pattern: Pattern3(match_str),
                    priority,
                    mode: mode.clone(),
                    body: PreparsedTemplate(body),
                    visibility,
                    from_import: false,
                    import_precedence: 0,
                };

                self.template_rules.entry(mode).or_default().push(rule);
            }
            BuilderState3::NamedTemplate { name, params } => {
                let visibility = Visibility::default();
                let as_type = None;

                let template = NamedTemplate3 {
                    params,
                    body: PreparsedTemplate(body),
                    visibility,
                    as_type,
                };

                self.named_templates.insert(name, Arc::new(template));
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_function_start(
        &mut self,
        attrs: OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(&attrs, b"name", b"xsl:function", pos, source)?;
        let as_type = get_attr_optional(&attrs, b"as")?.and_then(|s| self.parse_sequence_type(&s));

        self.features.uses_higher_order_functions = true;
        self.state_stack.push(BuilderState3::Function {
            name,
            params: Vec::new(),
            as_type,
        });
        Ok(())
    }

    pub(crate) fn handle_function_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::Function {
            name,
            params,
            as_type,
        } = current_state
        {
            let visibility = Visibility::default();

            let function = Function3 {
                name: name.clone(),
                params,
                body: PreparsedTemplate(body),
                as_type,
                visibility,
            };

            self.functions.insert(name, Arc::new(function));
        }
        Ok(())
    }

    pub(crate) fn handle_output(&mut self, attrs: &OwnedAttributes) -> Result<(), Xslt3Error> {
        let name = get_attr_optional(attrs, b"name")?;

        let mut output = if let Some(ref name) = name {
            self.outputs.get(name).cloned().unwrap_or_default()
        } else {
            self.output.clone()
        };

        if let Some(method) = get_attr_optional(attrs, b"method")? {
            output.method = Some(method);
        }
        if let Some(version) = get_attr_optional(attrs, b"version")? {
            output.version = Some(version);
        }
        if let Some(indent) = get_attr_optional(attrs, b"indent")? {
            output.indent = Some(indent == "yes");
        }
        if let Some(encoding) = get_attr_optional(attrs, b"encoding")? {
            output.encoding = Some(encoding);
        }
        if let Some(omit) = get_attr_optional(attrs, b"omit-xml-declaration")? {
            output.omit_xml_declaration = Some(omit == "yes");
        }
        if let Some(standalone) = get_attr_optional(attrs, b"standalone")? {
            output.standalone = Some(standalone);
        }
        if let Some(doctype_public) = get_attr_optional(attrs, b"doctype-public")? {
            output.doctype_public = Some(doctype_public);
        }
        if let Some(doctype_system) = get_attr_optional(attrs, b"doctype-system")? {
            output.doctype_system = Some(doctype_system);
        }
        if let Some(cdata) = get_attr_optional(attrs, b"cdata-section-elements")? {
            output.cdata_section_elements = cdata.split_whitespace().map(String::from).collect();
        }
        if let Some(html_version) = get_attr_optional(attrs, b"html-version")? {
            output.html_version = Some(html_version);
        }
        if let Some(include_ct) = get_attr_optional(attrs, b"include-content-type")? {
            output.include_content_type = Some(include_ct == "yes");
        }
        if let Some(media_type) = get_attr_optional(attrs, b"media-type")? {
            output.media_type = Some(media_type);
        }
        if let Some(normalization) = get_attr_optional(attrs, b"normalization-form")? {
            output.normalization_form = Some(normalization);
        }
        if let Some(suppress) = get_attr_optional(attrs, b"suppress-indentation")? {
            output.suppress_indentation = suppress.split_whitespace().map(String::from).collect();
        }
        if let Some(undeclare) = get_attr_optional(attrs, b"undeclare-prefixes")? {
            output.undeclare_prefixes = Some(undeclare == "yes");
        }
        if let Some(use_maps) = get_attr_optional(attrs, b"use-character-maps")? {
            output.use_character_maps = use_maps.split_whitespace().map(String::from).collect();
        }
        if let Some(bom) = get_attr_optional(attrs, b"byte-order-mark")? {
            output.byte_order_mark = Some(bom == "yes");
        }
        if let Some(escape) = get_attr_optional(attrs, b"escape-uri-attributes")? {
            output.escape_uri_attributes = Some(escape == "yes");
        }
        if let Some(build) = get_attr_optional(attrs, b"build-tree")? {
            output.build_tree = Some(build == "yes");
        }
        if let Some(sep) = get_attr_optional(attrs, b"item-separator")? {
            output.item_separator = Some(sep);
        }

        if let Some(name) = name {
            output.name = Some(name.clone());
            self.outputs.insert(name, output);
        } else {
            self.output = output;
        }
        Ok(())
    }

    pub(crate) fn handle_decimal_format(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_optional(attrs, b"name")?;
        let mut decl = crate::ast::DecimalFormatDeclaration {
            name: name.clone(),
            ..Default::default()
        };

        if let Some(s) = get_attr_optional(attrs, b"decimal-separator")?
            && let Some(c) = s.chars().next()
        {
            decl.decimal_separator = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"grouping-separator")?
            && let Some(c) = s.chars().next()
        {
            decl.grouping_separator = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"infinity")? {
            decl.infinity = s;
        }
        if let Some(s) = get_attr_optional(attrs, b"minus-sign")?
            && let Some(c) = s.chars().next()
        {
            decl.minus_sign = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"NaN")? {
            decl.nan = s;
        }
        if let Some(s) = get_attr_optional(attrs, b"percent")?
            && let Some(c) = s.chars().next()
        {
            decl.percent = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"per-mille")?
            && let Some(c) = s.chars().next()
        {
            decl.per_mille = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"zero-digit")?
            && let Some(c) = s.chars().next()
        {
            decl.zero_digit = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"digit")?
            && let Some(c) = s.chars().next()
        {
            decl.digit = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"pattern-separator")?
            && let Some(c) = s.chars().next()
        {
            decl.pattern_separator = c;
        }
        if let Some(s) = get_attr_optional(attrs, b"exponent-separator")?
            && let Some(c) = s.chars().next()
        {
            decl.exponent_separator = c;
        }

        self.decimal_formats.insert(name, decl);
        Ok(())
    }

    pub(crate) fn handle_namespace_alias(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let stylesheet_prefix = get_attr_required(
            attrs,
            b"stylesheet-prefix",
            b"xsl:namespace-alias",
            pos,
            source,
        )?;
        let result_prefix =
            get_attr_required(attrs, b"result-prefix", b"xsl:namespace-alias", pos, source)?;

        self.namespace_aliases.push(crate::ast::NamespaceAlias {
            stylesheet_prefix,
            result_prefix,
        });
        Ok(())
    }

    pub(crate) fn handle_use_package(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:use-package", pos, source)?;
        let version = get_attr_optional(attrs, b"package-version")?;

        self.use_packages.push(crate::ast::UsePackage {
            name,
            version,
            overrides: Vec::new(),
        });
        Ok(())
    }

    pub(crate) fn handle_context_item(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let as_type = get_attr_optional(attrs, b"as")?.and_then(|s| self.parse_sequence_type(&s));
        let use_when = get_attr_optional(attrs, b"use-when")?;

        self.context_item = Some(crate::ast::ContextItemDeclaration { as_type, use_when });
        Ok(())
    }

    pub(crate) fn handle_global_context_item(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let as_type = get_attr_optional(attrs, b"as")?.and_then(|s| self.parse_sequence_type(&s));
        let use_when = get_attr_optional(attrs, b"use-when")?;

        self.global_context_item =
            Some(crate::ast::GlobalContextItemDeclaration { as_type, use_when });
        Ok(())
    }

    pub(crate) fn handle_initial_template(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:initial-template", pos, source)?;
        self.initial_template = Some(crate::ast::InitialTemplateDeclaration { name });
        Ok(())
    }

    pub(crate) fn handle_simple_page_master(
        &mut self,
        attrs: &OwnedAttributes,
    ) -> Result<(), Xslt3Error> {
        let mut page = PageLayout::default();
        let mut master_name = None;

        for (key, val_bytes) in attrs {
            let key_str = from_utf8(key).map_err(|e| Xslt3Error::parse(e.to_string()))?;
            let val_str = from_utf8(val_bytes).map_err(|e| Xslt3Error::parse(e.to_string()))?;
            match key_str {
                "master-name" => master_name = Some(val_str.to_string()),
                "page-width" => {
                    page.size
                        .set_width(run_parser(parse_length, val_str).map_err(|e| {
                            Xslt3Error::parse(format!("Invalid page-width '{}': {}", val_str, e))
                        })?);
                }
                "page-height" => {
                    page.size
                        .set_height(run_parser(parse_length, val_str).map_err(|e| {
                            Xslt3Error::parse(format!("Invalid page-height '{}': {}", val_str, e))
                        })?);
                }
                "size" => {
                    page.size = parse_page_size(val_str).map_err(|e| {
                        Xslt3Error::parse(format!("Invalid page size '{}': {}", val_str, e))
                    })?;
                }
                "margin" => {
                    page.margins = Some(parse_shorthand_margins(val_str).map_err(|e| {
                        Xslt3Error::parse(format!("Invalid margin '{}': {}", val_str, e))
                    })?);
                }
                _ => {}
            }
        }

        let name = master_name.unwrap_or_else(|| "default".to_string());

        // Set as default if this is the first page master or if explicitly named "default"
        if self.stylesheet.default_page_master_name.is_none() || name == "default" {
            self.stylesheet.default_page_master_name = Some(name.clone());
        }

        self.stylesheet.page_masters.insert(name, page);
        Ok(())
    }

    pub(crate) fn handle_import(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let href = get_attr_required(attrs, b"href", b"xsl:import", pos, source)?;
        self.imports.push(crate::ast::ImportDeclaration { href });
        Ok(())
    }

    pub(crate) fn handle_include(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let href = get_attr_required(attrs, b"href", b"xsl:include", pos, source)?;
        self.includes.push(crate::ast::IncludeDeclaration { href });
        Ok(())
    }

    pub(crate) fn handle_key(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:key", pos, source)?;
        let match_pattern = get_attr_required(attrs, b"match", b"xsl:key", pos, source)?;
        let use_str = get_attr_required(attrs, b"use", b"xsl:key", pos, source)?;
        let use_expr = self.parse_xpath(&use_str)?;

        let collation = get_attr_optional(attrs, b"collation")?;
        let composite = get_attr_optional(attrs, b"composite")?
            .map(|s| s == "yes" || s == "true")
            .unwrap_or(false);

        let key = crate::ast::KeyDeclaration {
            name: name.clone(),
            match_pattern,
            use_expr,
            collation,
            composite,
        };

        self.keys.insert(name, key);
        Ok(())
    }

    pub(crate) fn handle_attribute_set_start(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:attribute-set", pos, source)?;
        let use_attribute_sets = get_attr_optional(attrs, b"use-attribute-sets")?
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();
        let visibility = get_attr_optional(attrs, b"visibility")?
            .map(|v| self.parse_visibility(&v))
            .unwrap_or_default();

        self.state_stack.push(BuilderState3::AttributeSet {
            name,
            use_attribute_sets,
            visibility,
        });
        Ok(())
    }

    pub(crate) fn handle_attribute_set_end(
        &mut self,
        current_state: BuilderState3,
        body: Vec<Xslt3Instruction>,
    ) -> Result<(), Xslt3Error> {
        if let BuilderState3::AttributeSet {
            name,
            use_attribute_sets,
            visibility,
        } = current_state
        {
            self.attribute_sets.insert(
                name.clone(),
                crate::ast::AttributeSet {
                    name,
                    use_attribute_sets,
                    attributes: PreparsedTemplate(body),
                    visibility,
                },
            );
        }
        Ok(())
    }

    pub(crate) fn handle_attribute_set_empty(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:attribute-set", pos, source)?;
        let use_attribute_sets = get_attr_optional(attrs, b"use-attribute-sets")?
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();
        let visibility = get_attr_optional(attrs, b"visibility")?
            .map(|v| self.parse_visibility(&v))
            .unwrap_or_default();

        self.attribute_sets.insert(
            name.clone(),
            crate::ast::AttributeSet {
                name,
                use_attribute_sets,
                attributes: PreparsedTemplate(vec![]),
                visibility,
            },
        );
        Ok(())
    }

    pub(crate) fn handle_preserve_space(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let elements = get_attr_required(attrs, b"elements", b"xsl:preserve-space", pos, source)?;
        for elem in elements.split_whitespace() {
            self.preserve_space.push(elem.to_string());
        }
        Ok(())
    }

    pub(crate) fn handle_strip_space(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let elements = get_attr_required(attrs, b"elements", b"xsl:strip-space", pos, source)?;
        for elem in elements.split_whitespace() {
            self.strip_space.push(elem.to_string());
        }
        Ok(())
    }

    pub(crate) fn handle_attribute_empty(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name_str = get_attr_required(attrs, b"name", b"xsl:attribute", pos, source)?;
        let name = self.parse_avt(&name_str)?;

        let body = if let Some(select_str) = get_attr_optional(attrs, b"select")? {
            let expr = self.parse_xpath(&select_str)?;
            PreparsedTemplate(vec![Xslt3Instruction::Sequence { select: expr }])
        } else {
            PreparsedTemplate(vec![])
        };

        let instr = Xslt3Instruction::Attribute { name, body };

        if let Some(parent) = self.instruction_stack.last_mut() {
            parent.push(instr);
        }
        Ok(())
    }

    pub(crate) fn handle_character_map_start(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let name = get_attr_required(attrs, b"name", b"xsl:character-map", pos, source)?;
        let use_character_maps = get_attr_optional(attrs, b"use-character-maps")?
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        self.state_stack
            .push(crate::compiler::BuilderState3::CharacterMap {
                name,
                use_character_maps,
                mappings: Vec::new(),
            });
        Ok(())
    }

    pub(crate) fn handle_character_map_end(
        &mut self,
        current_state: crate::compiler::BuilderState3,
    ) -> Result<(), Xslt3Error> {
        if let crate::compiler::BuilderState3::CharacterMap {
            name,
            use_character_maps,
            mappings,
        } = current_state
        {
            self.character_maps.insert(
                name.clone(),
                crate::ast::CharacterMap {
                    name,
                    use_character_maps,
                    mappings,
                },
            );
        }
        Ok(())
    }

    pub(crate) fn handle_output_character(
        &mut self,
        attrs: &OwnedAttributes,
        pos: usize,
        source: &str,
    ) -> Result<(), Xslt3Error> {
        let character_str =
            get_attr_required(attrs, b"character", b"xsl:output-character", pos, source)?;
        let string = get_attr_required(attrs, b"string", b"xsl:output-character", pos, source)?;

        let character = character_str.chars().next().ok_or_else(|| {
            Xslt3Error::parse("xsl:output-character requires a non-empty 'character' attribute")
        })?;

        let output_char = crate::ast::OutputCharacter { character, string };

        if let Some(crate::compiler::BuilderState3::CharacterMap { mappings, .. }) =
            self.state_stack.last_mut()
        {
            mappings.push(output_char);
        }
        Ok(())
    }
}
